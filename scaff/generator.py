"""Core generation logic using OpenAI to create agent specifications."""

import json
import os
import time
from typing import Any

import openai


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
        max_retries: Maximum number of retries on failure (default: 3)

    Returns:
        Parsed JSON object with agent configuration

    Raises:
        ValueError: If API key not set, API fails, or response is invalid
    """
    api_key = os.environ.get("OPENAI_API_KEY")
    if not api_key:
        raise ValueError(
            "OPENAI_API_KEY environment variable not set. "
            "Set it with: export OPENAI_API_KEY='your-key' (Linux/Mac) "
            "or $env:OPENAI_API_KEY='your-key' (Windows)"
        )

    client = openai.OpenAI(api_key=api_key)

    system_prompt = (
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

    if verbose:
        print("[*] Sending description to OpenAI...")

    last_error = None

    for attempt in range(max_retries):
        try:
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

            # Clean up markdown wrapping if present
            content = _extract_json(content)

            try:
                agent_spec = json.loads(content)
            except json.JSONDecodeError as e:
                raise ValueError(
                    f"Failed to parse OpenAI response as JSON: {e}\n\nResponse:\n{content}"
                )

            # Validate the spec
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
        except ValueError as e:
            last_error = e
            if attempt < max_retries - 1 and "JSON" in str(e):
                if verbose:
                    print(f"[!] Parse error (attempt {attempt + 1}/{max_retries}): {e}")
                    print("[*] Retrying...")
                time.sleep(1)
                continue
            raise

    raise ValueError(
        f"Failed to generate agent specification after {max_retries} attempts. "
        f"Last error: {last_error}"
    )
