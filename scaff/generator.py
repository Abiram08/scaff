"""Core generation logic using OpenAI to create agent specifications."""

import json
import os
import random
import re
import time
import traceback
from typing import Any

import openai

from . import cache as scache
from .config import ScaffConfig
from .token_counter import TokenCounter
from .request_enforcer import Mode, ModeRegistry, RequestEnforcer
from .token_tracker import TokenTracker
from .session_manager import SessionManager
from .telemetry import Telemetry

IDENTIFIER_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]{0,63}$")

# Approximate pricing per 1K tokens (USD) for common models
MODEL_PRICING: dict[str, tuple[float, float]] = {
    "gpt-4o": (2.50, 10.00),
    "gpt-4o-mini": (0.15, 0.60),
    "gpt-4-turbo": (10.00, 30.00),
    "gpt-3.5-turbo": (0.50, 1.50),
}
DEFAULT_MAX_TOKENS = 4096
ECONOMY_MAX_TOKENS = 2048


def _count_tokens(text: str) -> int:
    """Estimate token count (~4 chars per token for English text)."""
    return max(1, len(text) // 4)


def _estimate_cost(
    input_text: str,
    output_tokens: int,
    model: str,
) -> float:
    """Estimate cost in USD for a request."""
    pricing = MODEL_PRICING.get(model, (2.50, 10.00))
    input_cost = (_count_tokens(input_text) / 1000) * pricing[0]
    output_cost = (output_tokens / 1000) * pricing[1]
    return round(input_cost + output_cost, 5)


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
        "\n\nIMPORTANT: Tool names determine which auto-implementation is generated."
        "\n  - Names containing 'search', 'fetch', 'lookup', 'find', 'query' → HTTP GET implementation"
        "\n  - Names containing 'read', 'load', 'open', 'parse' → file reading implementation"
        "\n  - Names containing 'write', 'save', 'store', 'log' → file writing implementation"
        "\n  - Names containing 'send', 'notify', 'alert', 'message' → print/stdout implementation"
        "\n  - Names containing 'analyze', 'count', 'summarize', 'compute' → data analysis"
        "\n  - Names containing 'date', 'time', 'schedule' → datetime implementation"
        "\n  - Names containing 'list', 'display', 'get_' → enumeration implementation"
        "\n  - Descriptions mentioning 'weather' or 'forecast' → wttr.in weather API"
        "\n  - Descriptions mentioning 'github', 'issue', 'repo' → GitHub REST API"
        "\nUse descriptive names that match these patterns for best auto-implementation."
        "\n\nRequired JSON structure: {"
        "\n  'agent_name': 'kebab-case-name',"
        "\n  'description': 'description',"
        "\n  'system_prompt': 'system prompt for the agent',"
        "\n  'tools': [{'name': 'tool_name', 'description': 'desc', 'parameters': {...}}],"
        "\n  'dependencies': ['package1', 'package2'],"
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
    max_tokens: int = DEFAULT_MAX_TOKENS,
    temperature: float = 0.7,
    verbose: bool = False,
) -> tuple[str, int, dict[str, str]]:
    """Request an agent spec from OpenAI using streaming.

    Returns (raw_text, exact_token_count, response_headers).
    Streaming enables real-time token counting and lower perceived latency.
    """
    response = client.chat.completions.create(
        model=model,
        max_tokens=max_tokens,
        temperature=temperature,
        messages=[
            {"role": "system", "content": system_prompt},
            {
                "role": "user",
                "content": f"Create an AI agent with the following description:\n\n{description}",
            },
        ],
        stream=True,
    )

    # Extract headers before iterating the stream
    headers = {}
    try:
        raw = response._response
        for k, v in raw.headers.items():
            if k.startswith("x-ratelimit-"):
                headers[k] = v
    except (AttributeError, TypeError):
        pass

    collected_pieces = []
    char_count = 0
    exact_tokens = 0

    for chunk in response:
        delta = chunk.choices[0].delta if chunk.choices else None
        if delta and delta.content:
            piece = delta.content
            collected_pieces.append(piece)
            char_count += len(piece)
            if verbose and char_count > 0 and char_count % 160 == 0:
                estimated = max(1, char_count // 4)
                print(f"[*] Streaming: ~{estimated} tokens received...", end="\r")

        if hasattr(chunk, 'usage') and chunk.usage:
            exact_tokens = chunk.usage.completion_tokens or 0

    if verbose and char_count > 0:
        print()

    content = "".join(collected_pieces)
    if not content:
        raise ValueError("OpenAI returned empty response (streaming)")

    # Prefer exact count from API, fall back to char-based estimate
    output_tokens = exact_tokens if exact_tokens > 0 else max(1, char_count // 4)

    return content, output_tokens, headers


def generate_agent_spec(
    description: str,
    model: str = "gpt-4o",
    verbose: bool = False,
    max_retries: int = 3,
    cheap: bool = False,
    show_cost: bool = False,
    no_cache: bool = False,
) -> dict[str, Any]:
    """
    Generate an agent specification from a plain English description.

    Args:
        description: Plain English description of what the agent should do
        model: OpenAI model to use (default: gpt-4o)
        verbose: Show generation steps
        max_retries: Maximum number of retries on API failure (default: 3)
        cheap: Use gpt-4o-mini to save tokens/cost
        show_cost: Print estimated cost before calling API
        no_cache: Bypass cache and force a fresh API call

    Returns:
        Parsed JSON object with agent configuration

    Raises:
        ValueError: If API key not set, API fails, or response is invalid
    """
    # Initialize session and token management
    session_manager = SessionManager()
    token_counter = TokenCounter()
    token_tracker = TokenTracker()
    
    # Read saved API mode from config (min/medium/max)
    mode_str = ScaffConfig.get("api_mode", "medium")
    try:
        current_mode = Mode(mode_str)
    except ValueError:
        current_mode = Mode.MEDIUM
    mode_config = ModeRegistry.get_config(current_mode)
    request_enforcer = RequestEnforcer(mode=current_mode)
    
    api_key = os.environ.get("OPENAI_API_KEY")
    if not api_key:
        raise ValueError(
            "OPENAI_API_KEY environment variable not set.\n"
            "export OPENAI_API_KEY='your-key'"
        )

    # Model: --cheap overrides mode, otherwise use mode's model
    if cheap:
        model = "gpt-4o-mini"
    else:
        model = mode_config.model

    max_tokens = mode_config.max_tokens
    temperature = mode_config.temperature

    client = openai.OpenAI(api_key=api_key)

    full_prompt = _build_system_prompt(strict_json=False)
    user_message = f"Create an AI agent with the following description:\n\n{description}"
    input_text = full_prompt + "\n" + user_message

    # Count input tokens with the new token counter
    input_tokens = token_counter.count_tokens(input_text)
    estimated_output_tokens = token_counter.estimate_response_tokens(
        input_tokens, max_tokens
    )
    
    # Check memory limits
    memory_check = session_manager.check_memory_limit()
    if memory_check["exceeds_limit"]:
        raise ValueError(
            f"Memory limit exceeded ({memory_check['current_mb']:.1f}MB > {memory_check['limit_mb']}MB). "
            f"Try reducing input size or restart scaff."
        )
    
    # Check request limits
    try:
        request_enforcer.check_budget(input_tokens + estimated_output_tokens)
        request_enforcer.check_rate_limit()
    except ValueError as e:
        raise ValueError(f"API limit exceeded: {e}") from e

    # Show cost estimate
    if show_cost:
        cost = _estimate_cost(input_text, max_tokens, model)
        print(f"[i] Mode: {current_mode.value.upper()}")
        print(f"[i] Model: {model}")
        print(f"[i] Temperature: {temperature}")
        print(f"[i] Input: ~{input_tokens} tokens")
        print(f"[i] Est. output: ~{estimated_output_tokens} tokens")
        print(f"[i] Est. cost: ${cost:.5f}")

    # Check cache
    if not no_cache:
        cached = scache.get(description, model)
        if cached:
            try:
                agent_spec = json.loads(_extract_json(cached))
                agent_spec = normalize_agent_spec(agent_spec)
                validate_agent_spec(agent_spec)
                if verbose:
                    print(f"[OK] Using cached response ({agent_spec.get('agent_name', 'unknown')})")
                token_tracker.end_session()
                return agent_spec
            except (json.JSONDecodeError, ValueError):
                if verbose:
                    print("[!] Cache corrupted, re-fetching...")

    if verbose:
        print("[*] Sending description to OpenAI...")

    last_error = None

    for attempt in range(max_retries):
        try:
            content, output_tokens, headers = _request_agent_spec(
                client,
                description,
                model,
                full_prompt,
                max_tokens=max_tokens,
                temperature=temperature,
                verbose=verbose,
            )

            # Update rate limiter with real server headers
            request_enforcer.update_from_headers(headers)
            
            # Track this request
            token_tracker.record_request(
                session_id=session_manager.session_id,
                model=model,
                input_tokens=input_tokens,
                output_tokens=output_tokens,
                cost=_estimate_cost(input_text, output_tokens, model),
            )
            
            # Record request in enforcer
            request_enforcer.record_request(input_tokens + output_tokens)
            session_manager.request_count += 1

            # Cache the successful response
            scache.set(description, model, content)

            try:
                agent_spec = json.loads(_extract_json(content))
            except json.JSONDecodeError as e:
                if verbose:
                    print(f"[!] JSON parse failed: {e}")
                    print("[*] Retrying once with stricter JSON prompt...")

                strict_prompt = _build_system_prompt(strict_json=True)
                strict_content, strict_output_tokens, _ = _request_agent_spec(
                    client,
                    description,
                    model,
                    strict_prompt,
                    max_tokens=max_tokens,
                    temperature=temperature,
                    verbose=verbose,
                )
                token_tracker.record_request(
                    session_id=session_manager.session_id,
                    model=model,
                    input_tokens=input_tokens,
                    output_tokens=strict_output_tokens,
                    cost=_estimate_cost(input_text, strict_output_tokens, model),
                )
                request_enforcer.record_request(input_tokens + strict_output_tokens)
                
                scache.set(description, model, strict_content)
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
                print(f"[i] Tokens used: {input_tokens + output_tokens:,}")
                print(f"[i] Session: {session_manager.session_id}")

            # End session to persist token tracking
            token_tracker.end_session()
            return agent_spec

        except openai.RateLimitError as e:
            last_error = e
            telemetry = Telemetry()
            telemetry.report_error(
                error_type="RateLimitError",
                error_message=str(e),
                stack_trace=traceback.format_exc(),
                context={
                    "model": model,
                    "attempt": attempt + 1,
                    "max_retries": max_retries,
                },
            )
            # Parse server rate limit headers for optimal retry timing
            server_wait = 0.0
            try:
                raw_headers = dict(e.response.headers)
                request_enforcer.update_from_headers(raw_headers)
                server_wait = request_enforcer.bucket.estimated_wait()
            except (AttributeError, KeyError, ValueError, TypeError):
                pass
            if attempt < max_retries - 1:
                wait_time = max(server_wait, min(2 ** attempt * (1 + random.uniform(0, 1)), 60.0))
                if verbose:
                    print(f"[!] Rate limited (attempt {attempt + 1}/{max_retries}): {e}")
                    print(f"[*] Retrying in {wait_time:.1f} seconds...")
                time.sleep(wait_time)
            continue
        except openai.APIError as e:
            last_error = e
            
            # Log error to telemetry
            telemetry = Telemetry()
            telemetry.report_error(
                error_type="OpenAIAPIError",
                error_message=str(e),
                stack_trace=traceback.format_exc(),
                context={
                    "model": model,
                    "attempt": attempt + 1,
                    "max_retries": max_retries,
                },
            )
            
            if attempt < max_retries - 1:
                wait_time = min(2 ** attempt * (1 + random.uniform(0, 1)), 60.0)
                if verbose:
                    print(f"[!] API error (attempt {attempt + 1}/{max_retries}): {e}")
                    print(f"[*] Retrying in {wait_time:.1f} seconds...")
                time.sleep(wait_time)
            continue
        except ValueError as e:
            # Log validation errors
            telemetry = Telemetry()
            telemetry.report_error(
                error_type="ValidationError",
                error_message=str(e),
                stack_trace=traceback.format_exc(),
                context={
                    "model": model,
                    "attempt": attempt + 1,
                },
            )
            raise
        except Exception as e:
            # Catch any other unexpected errors
            telemetry = Telemetry()
            telemetry.report_error(
                error_type=type(e).__name__,
                error_message=str(e),
                stack_trace=traceback.format_exc(),
                context={"model": model},
            )
            raise

    # All retries failed
    telemetry = Telemetry()
    telemetry.report_error(
        error_type="MaxRetriesExceeded",
        error_message=f"Failed to generate agent specification after {max_retries} attempts",
        context={
            "model": model,
            "max_retries": max_retries,
            "last_error": str(last_error),
        },
    )
    
    raise ValueError(
        f"Failed to generate agent specification after {max_retries} attempts. "
        f"Last error: {last_error}"
    )
