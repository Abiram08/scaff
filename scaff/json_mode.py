"""JSON-protocol bridge for the Rust CLI hybrid architecture.

Reads a JSON command from stdin, executes it against the Python backend,
and writes a JSON response to stdout. This is the interface between
the Rust CLI frontend and the Python generation backend.
"""

import json
import sys
import traceback


def cmd_generate(request: dict) -> dict:
    from .generator import generate_agent_spec

    spec = generate_agent_spec(
        description=request["description"],
        model=request.get("model", "gpt-4o"),
        verbose=request.get("verbose", False),
        cheap=request.get("cheap", False),
        show_cost=request.get("show_cost", False),
        no_cache=request.get("no_cache", False),
    )
    return {"status": "ok", "agent_spec": spec}


def cmd_generate_and_write(request: dict) -> dict:
    from .generator import generate_agent_spec
    from .writer import write_generated_files

    spec = generate_agent_spec(
        description=request["description"],
        model=request.get("model", "gpt-4o"),
        verbose=request.get("verbose", False),
        cheap=request.get("cheap", False),
        show_cost=request.get("show_cost", False),
        no_cache=request.get("no_cache", False),
    )
    created_files = write_generated_files(
        spec,
        output_dir=request.get("output", "agent-output"),
        provider=request.get("provider", "openai"),
        ui_mode=request.get("ui_mode"),
        schedule=request.get("schedule"),
        memory=request.get("memory"),
        verbose=request.get("verbose", False),
    )
    return {
        "status": "ok",
        "agent_spec": spec,
        "created_files": created_files,
    }


def cmd_validate(request: dict) -> dict:
    from .generator import generate_agent_spec

    try:
        spec = generate_agent_spec(
            description=request["description"],
            verbose=True,
        )
        return {
            "status": "ok",
            "valid": True,
            "agent_name": spec.get("agent_name"),
            "description": spec.get("description"),
            "tool_count": len(spec.get("tools", [])),
            "dependencies": spec.get("dependencies", []),
        }
    except Exception as e:
        return {"status": "error", "error": str(e)}


def cmd_estimate(request: dict) -> dict:
    from .request_enforcer import Mode, ModeRegistry
    from .token_counter import TokenCounter
    from .pricing import PricingCalculator
    from .generator import _build_system_prompt

    counter = TokenCounter()
    system_prompt = _build_system_prompt(strict_json=False)
    user_message = f"Create an AI agent with the following description:\n\n{request['description']}"
    input_text = system_prompt + "\n" + user_message
    full_input_tokens = counter.count_tokens(input_text)

    modes = []
    for mode in [Mode.MIN, Mode.MEDIUM, Mode.MAX]:
        config = ModeRegistry.get_config(mode)
        avg_output = config.max_tokens // 2
        input_cost = PricingCalculator.calculate_request_cost(
            config.model, full_input_tokens, 0
        )
        total_cost = PricingCalculator.calculate_request_cost(
            config.model, full_input_tokens, avg_output
        )
        modes.append({
            "mode": mode.value,
            "model": config.model,
            "temperature": config.temperature,
            "max_tokens": config.max_tokens,
            "input_cost": round(input_cost, 5),
            "total_cost": round(total_cost, 5),
            "monthly_budget": config.monthly_cost,
            "monthly_tokens": config.monthly_tokens,
        })

    return {
        "status": "ok",
        "description": request["description"],
        "input_tokens": full_input_tokens,
        "modes": modes,
    }


COMMANDS = {
    "generate": cmd_generate,
    "generate_and_write": cmd_generate_and_write,
    "validate": cmd_validate,
    "estimate": cmd_estimate,
}


def main() -> None:
    if len(sys.argv) > 1 and sys.argv[1] == "--help-json":
        print("JSON protocol commands:", ", ".join(COMMANDS.keys()))
        sys.exit(0)

    try:
        raw = sys.stdin.read()
        if not raw.strip():
            json.dump({"status": "error", "error": "Empty stdin"}, sys.stdout)
            sys.exit(1)

        request = json.loads(raw)
        command = request.get("command", "")
        handler = COMMANDS.get(command)

        if not handler:
            json.dump({
                "status": "error",
                "error": f"Unknown command: {command}",
                "valid_commands": list(COMMANDS.keys()),
            }, sys.stdout)
            sys.exit(1)

        result = handler(request)
        json.dump(result, sys.stdout, default=str)

    except json.JSONDecodeError as e:
        json.dump({"status": "error", "error": f"Invalid JSON: {e}"}, sys.stdout)
        sys.exit(1)
    except Exception as e:
        json.dump({
            "status": "error",
            "error": str(e),
            "traceback": traceback.format_exc(),
        }, sys.stdout)
        sys.exit(1)


if __name__ == "__main__":
    main()
