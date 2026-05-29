"""Core generation logic using OpenAI to create agent specifications."""

import json
import os
import re
import time
from typing import Any

import openai

IDENTIFIER_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]{0,63}$")


def _identifier(value: str, fallback: str = "tool") -> str:
    """Return a Python/OpenAI-compatible function or parameter name."""
    name = re.sub(r"\W+", "_", value.strip().lower()).strip("_")
    if not name:
        name = fallback
    if name[0].isdigit():
        name = f"{fallback}_{name}"
    return name[:64]


def _kebab_name(value: str) -> str:
    """Return a readable kebab-case name for agent metadata."""
    words = re.findall(r"[A-Za-z0-9]+", value.lower())
    return "-".join(words[:6]) or "custom-agent"


def _dedupe_name(name: str, seen: set[str]) -> str:
    """Avoid duplicate generated names while keeping them short."""
    candidate = name
    suffix = 2
    while candidate in seen:
        suffix_text = f"_{suffix}"
        candidate = f"{name[:64 - len(suffix_text)]}{suffix_text}"
        suffix += 1
    seen.add(candidate)
    return candidate


def _normalize_parameters(parameters: Any) -> tuple[dict[str, dict[str, str]], list[str]]:
    """Normalize model parameter output to the template's expected shape."""
    if not parameters:
        return {}, []

    if not isinstance(parameters, dict):
        raise ValueError(f"Tool parameters must be an object. Got: {parameters}")

    if parameters.get("type") == "object" and isinstance(parameters.get("properties"), dict):
        raw_properties = parameters["properties"]
        raw_required = parameters.get("required", list(raw_properties.keys()))
        required = raw_required if isinstance(raw_required, list) else list(raw_properties.keys())
    else:
        raw_properties = parameters
        required = list(raw_properties.keys())

    normalized: dict[str, dict[str, str]] = {}
    required_names: list[str] = []
    seen: set[str] = set()

    for original_name, original_info in raw_properties.items():
        if not isinstance(original_info, dict):
            raise ValueError(f"Parameter '{original_name}' must be an object. Got: {original_info}")

        param_name = _dedupe_name(_identifier(str(original_name), fallback="param"), seen)
        normalized[param_name] = {
            "type": str(original_info.get("type", "string")),
            "description": str(
                original_info.get("description", f"Value for {original_name}")
            ),
        }
        if original_name in required:
            required_names.append(param_name)

    return normalized, required_names


def normalize_agent_spec(spec: dict[str, Any]) -> dict[str, Any]:
    """Normalize model output into runnable scaff templates."""
    normalized = dict(spec)
    normalized["agent_name"] = _kebab_name(
        str(normalized.get("agent_name") or normalized.get("description") or "custom-agent")
    )
    normalized["description"] = str(normalized.get("description") or normalized["agent_name"])
    normalized["system_prompt"] = str(
        normalized.get("system_prompt") or "You are a helpful AI agent."
    )

    dependencies = normalized.get("dependencies", [])
    if isinstance(dependencies, str):
        dependencies = [dependencies]
    elif not isinstance(dependencies, list):
        dependencies = []
    normalized["dependencies"] = [str(dep) for dep in dependencies]

    seen_tools: set[str] = set()
    tools: list[dict[str, Any]] = []
    for raw_tool in normalized.get("tools", []):
        tool = raw_tool.get("function", raw_tool) if isinstance(raw_tool, dict) else raw_tool
        if not isinstance(tool, dict):
            raise ValueError(f"Tool must be an object. Got: {tool}")

        tool_name = _dedupe_name(_identifier(str(tool.get("name", ""))), seen_tools)
        parameters, required = _normalize_parameters(tool.get("parameters", {}))
        tools.append(
            {
                "name": tool_name,
                "description": str(tool.get("description", f"Run {tool_name}")),
                "parameters": parameters,
                "required": required,
            }
        )

    normalized["tools"] = tools
    return normalized


def validate_agent_spec(spec: dict[str, Any]) -> bool:
    """
    Validate that the agent specification has all required fields.

    Args:
        spec: Agent specification to validate

    Returns:
        True if valid, raises ValueError otherwise
    """
    required_fields = ["agent_name", "description", "system_prompt", "tools", "dependencies"]

    for field in required_fields:
        if field not in spec:
            raise ValueError(f"Missing required field in agent spec: {field}")

    if not isinstance(spec.get("tools"), list) or len(spec["tools"]) == 0:
        raise ValueError("Agent specification must have at least one tool")

    for tool in spec["tools"]:
        missing = [f for f in ("name", "description", "parameters") if f not in tool]
        if missing:
            raise ValueError(f"Tool missing required fields: {missing}. Got: {tool}")
        if not IDENTIFIER_RE.match(str(tool["name"])):
            raise ValueError(f"Tool name must be a valid identifier. Got: {tool['name']}")
        if not isinstance(tool["parameters"], dict):
            raise ValueError(f"Tool parameters must be a mapping. Got: {tool['parameters']}")
        for param_name in tool["parameters"]:
            if not IDENTIFIER_RE.match(str(param_name)):
                raise ValueError(f"Parameter name must be a valid identifier. Got: {param_name}")

    return True


def _extract_json(text: str) -> str:
    """Extract JSON from a text that may contain markdown code blocks."""
    text = text.strip()
    if text.startswith("```"):
        parts = text.split("```")
        if len(parts) >= 3:
            text = parts[1]
            if text.startswith("json"):
                text = text[4:]
    return text.strip()


def _build_system_prompt(strict_json: bool = False) -> str:
    """Build the system prompt used to ask OpenAI for an agent spec."""
    prompt = (
        "You are an expert AI agent architect. Given a plain English description of what an AI agent "
        "should do, return a structured JSON object describing the agent's name, system prompt, tools "
        "with their parameters, MCP config, and Python dependencies. "
        "\n\nRequired JSON structure: {"
        "\n  'agent_name': 'kebab-case-name',"
        "\n  'description': 'description',"
        "\n  'system_prompt': 'system prompt for the agent',"
        "\n  'tools': [{'name': 'tool_name', 'description': 'desc', 'parameters': {...}}],"
        "\n  'dependencies': ['package1', 'package2'],"
        "\n  'mcp_config': {'name': 'name', 'version': '1.0.0', 'tools': [...]}"
        "\n}"
        "\n\nReturn ONLY valid JSON, no markdown, no explanation."
    )

    if strict_json:
        prompt += "\n\nreturn ONLY raw JSON, no markdown, no backticks, no explanation"

    return prompt


def _request_agent_spec(
    client: openai.OpenAI,
    description: str,
    model: str,
    system_prompt: str,
) -> str:
    """Request an agent spec from OpenAI and return the raw text response."""
    response = client.chat.completions.create(
        model=model,
        max_tokens=4096,
        temperature=0.1,
        messages=[
            {"role": "system", "content": system_prompt},
            {
                "role": "user",
                "content": f"Create an AI agent with the following description:\n\n{description}",
            },
        ],
    )

    content = response.choices[0].message.content
    if not content:
        raise ValueError("OpenAI returned empty response")

    return content


def generate_agent_spec(
    description: str,
    model: str = "gpt-4o",
    verbose: bool = False,
    max_retries: int = 3,
) -> dict[str, Any]:
    """
    Generate an agent specification from a plain English description.

    Args:
        description: Plain English description of what the agent should do
        model: OpenAI model to use (default: gpt-4o)
        verbose: Show generation steps
        max_retries: Maximum number of retries on API failure (default: 3)

    Returns:
        Parsed JSON object with agent configuration

    Raises:
        ValueError: If API key not set, API fails, or response is invalid
    """
    api_key = os.environ.get("OPENAI_API_KEY")
    if not api_key:
        raise ValueError(
            "OPENAI_API_KEY environment variable not set.\n"
            "export OPENAI_API_KEY='your-key'"
        )

    client = openai.OpenAI(api_key=api_key)

    if verbose:
        print("[*] Sending description to OpenAI...")

    last_error = None

    for attempt in range(max_retries):
        try:
            content = _request_agent_spec(
                client,
                description,
                model,
                _build_system_prompt(strict_json=False),
            )

            try:
                agent_spec = json.loads(_extract_json(content))
            except json.JSONDecodeError as e:
                if verbose:
                    print(f"[!] JSON parse failed: {e}")
                    print("[*] Retrying once with stricter JSON prompt...")

                strict_content = _request_agent_spec(
                    client,
                    description,
                    model,
                    _build_system_prompt(strict_json=True),
                )
                try:
                    agent_spec = json.loads(_extract_json(strict_content))
                except json.JSONDecodeError as strict_error:
                    raise ValueError(
                        "Failed to parse OpenAI response as JSON after strict retry: "
                        f"{strict_error}\n\nResponse:\n{strict_content}"
                    ) from strict_error

            agent_spec = normalize_agent_spec(agent_spec)
            validate_agent_spec(agent_spec)

            if verbose:
                print(f"[OK] Agent specification generated: {agent_spec.get('agent_name', 'unknown')}")

            return agent_spec

        except openai.APIError as e:
            last_error = e
            if attempt < max_retries - 1:
                wait_time = 2 ** attempt
                if verbose:
                    print(f"[!] API error (attempt {attempt + 1}/{max_retries}): {e}")
                    print(f"[*] Retrying in {wait_time} seconds...")
                time.sleep(wait_time)
            continue
        except ValueError:
            raise

    raise ValueError(
        f"Failed to generate agent specification after {max_retries} attempts. "
        f"Last error: {last_error}"
    )
