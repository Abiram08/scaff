use std::io::{self, Write};

use console::style;

use crate::display;
use crate::python_bridge::{self, GenerateRequest};

pub fn run_repl() -> Result<(), String> {
    println!();
    println!("{}", style("scaff interactive shell").cyan().bold());
    println!("{}", style("Type 'help' for commands, 'exit' to quit.").dim());
    println!();

    let mut current_desc: Option<String> = None;

    loop {
        print!("{} ", style("scaff>").cyan().bold());
        io::stdout().flush().map_err(|e| format!("IO error: {e}"))?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| format!("Read error: {e}"))?;

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let command = parts[0].to_lowercase();
        let args = parts.get(1).map(|s| s.trim()).unwrap_or("");

        match command.as_str() {
            "exit" | "quit" => {
                println!("Goodbye!");
                break;
            }
            "help" => print_help(),
            "describe" | "desc" => {
                if args.is_empty() {
                    println!("  {} Usage: describe <plain english description of agent>", style("!").yellow());
                } else {
                    current_desc = Some(args.to_string());
                    println!("  {} Description set to: \"{}\"", style("OK").green(), args);
                    validate_current(&current_desc);
                }
            }
            "validate" | "test" => {
                if let Some(ref desc) = current_desc {
                    validate_current(&Some(desc.clone()));
                } else if !args.is_empty() {
                    current_desc = Some(args.to_string());
                    validate_current(&current_desc);
                } else {
                    println!("  {} No description set. Use 'describe <text>' first.", style("!").yellow());
                }
            }
            "generate" | "gen" => {
                let desc = if !args.is_empty() {
                    Some(args.to_string())
                } else {
                    current_desc.clone()
                };
                match desc {
                    Some(d) => {
                        current_desc = Some(d.clone());
                        generate_agent(&d)?;
                    }
                    None => {
                        println!("  {} No description set. Use 'describe <text>' or 'generate <text>'.", style("!").yellow());
                    }
                }
            }
            "show" => {
                match &current_desc {
                    Some(d) => {
                        println!("  {} Current description: \"{}\"", style("info").cyan(), d);
                    }
                    None => {
                        println!("  {} No description set. Use 'describe <text>'.", style("!").yellow());
                    }
                }
            }
            "clear" => {
                current_desc = None;
                println!("  {} Description cleared.", style("OK").green());
            }
            "estimate" | "cost" => {
                let desc = if !args.is_empty() {
                    Some(args.to_string())
                } else {
                    current_desc.clone()
                };
                match desc {
                    Some(d) => {
                        estimate_cost(&d)?;
                    }
                    None => {
                        println!("  {} No description set.", style("!").yellow());
                    }
                }
            }
            _ => {
                println!("  {} Unknown command: {}. Type 'help' for available commands.", style("ERROR").red(), command);
            }
        }
    }

    Ok(())
}

fn print_help() {
    println!();
    println!("{}", style("Available commands:").bold());
    println!("  {:<25} {}", style("describe <text>").cyan(), "Set or change the agent description");
    println!("  {:<25} {}", style("validate / test").cyan(), "Validate the current description");
    println!("  {:<25} {}", style("generate / gen").cyan(), "Generate the agent from current description");
    println!("  {:<25} {}", style("generate <text>").cyan(), "Set description and generate in one step");
    println!("  {:<25} {}", style("estimate / cost").cyan(), "Preview cost for current description");
    println!("  {:<25} {}", style("show").cyan(), "Show the current description");
    println!("  {:<25} {}", style("clear").cyan(), "Clear the current description");
    println!("  {:<25} {}", style("help").cyan(), "Show this help message");
    println!("  {:<25} {}", style("exit / quit").cyan(), "Exit the shell");
    println!();
}

fn validate_current(desc: &Option<String>) {
    if let Some(ref d) = desc {
        let spinner = display::create_spinner("Validating...");
        let req = GenerateRequest {
            command: "validate".to_string(),
            description: d.clone(),
            model: None,
            provider: None,
            output: None,
            ui_mode: None,
            schedule: None,
            memory: None,
            cheap: false,
            verbose: false,
            show_cost: false,
            no_cache: false,
        };
        match python_bridge::call_generate(&req) {
            Ok(response) => {
                spinner.finish_and_clear();
                if response.valid.unwrap_or(false) {
                    display::ok("Description works!");
                    if let Some(name) = &response.agent_name {
                        println!("       Agent: {}", name);
                    }
                    if let Some(count) = response.tool_count {
                        println!("       Tools: {count}");
                    }
                } else {
                    display::warn("Description may not work optimally");
                }
            }
            Err(e) => {
                spinner.finish_and_clear();
                display::err(&format!("Validation failed: {e}"));
            }
        }
    }
}

fn generate_agent(description: &str) -> Result<(), String> {
    let spinner = display::create_spinner("Generating your agent...");
    let req = GenerateRequest {
        command: "generate".to_string(),
        description: description.to_string(),
        model: None,
        provider: None,
        output: None,
        ui_mode: None,
        schedule: None,
        memory: None,
        cheap: false,
        verbose: false,
        show_cost: false,
        no_cache: false,
    };
    let response = python_bridge::call_generate(&req)?;
    spinner.finish_and_clear();

    if response.status == "error" {
        return Err(response.error.unwrap_or_else(|| "Generation failed".to_string()));
    }

    display::ok("Agent spec generated!");
    println!();

    if let Some(spec) = &response.agent_spec {
        if let Some(name) = spec.get("agent_name").and_then(|v| v.as_str()) {
            display::kv("Agent Name", name);
        }
        if let Some(desc) = spec.get("description").and_then(|v| v.as_str()) {
            display::kv("Description", desc);
        }
        if let Some(tools) = spec.get("tools").and_then(|v| v.as_array()) {
            display::kv("Tools", &format!("{} tool(s)", tools.len()));
            for t in tools {
                if let Some(tname) = t.get("name").and_then(|v| v.as_str()) {
                    let tdesc = t.get("description").and_then(|v| v.as_str()).unwrap_or("No description");
                    println!("       - {}: {}", style(tname).green(), style(tdesc).dim());
                }
            }
        }
        if let Some(deps) = spec.get("dependencies").and_then(|v| v.as_array()) {
            display::kv("Dependencies", &deps.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", "));
        }
    }

    if let Some(spec) = &response.agent_spec {
        if let Some(sp) = spec.get("system_prompt").and_then(|v| v.as_str()) {
            println!();
            println!("  {}", style("System Prompt:").cyan().bold());
            println!("  {}",
                style(sp.chars().take(200).collect::<String>()).dim(),
            );
            if sp.len() > 200 {
                println!("  {}...", style("[truncated]").dim());
            }
        }
    }

    println!();
    println!("  {} Run 'scaff create \"{}\"' to generate the full project.",
        style("Tip:").cyan().dim(),
        description,
    );
    println!();

    Ok(())
}

fn estimate_cost(description: &str) -> Result<(), String> {
    let spinner = display::create_spinner("Estimating costs...");
    let req = GenerateRequest {
        command: "estimate".to_string(),
        description: description.to_string(),
        model: None,
        provider: None,
        output: None,
        ui_mode: None,
        schedule: None,
        memory: None,
        cheap: false,
        verbose: false,
        show_cost: false,
        no_cache: false,
    };
    let response = python_bridge::call_generate(&req)?;
    spinner.finish_and_clear();

    if response.status == "error" {
        return Err(response.error.unwrap_or_else(|| "Estimate failed".to_string()));
    }

    if let Some(modes_val) = response.modes {
        if let Some(modes) = modes_val.as_array() {
            println!();
            println!("  {:<8} {:<20} {:<15} {:<15}", "Mode", "Model", "Input Cost", "Est. Total");
            println!("  {}", "-".repeat(60));
            for m in modes {
                let mode_name = m["mode"].as_str().unwrap_or("?").to_uppercase();
                let model = m["model"].as_str().unwrap_or("?");
                let input_cost = m["input_cost"].as_f64().unwrap_or(0.0);
                let total_cost = m["total_cost"].as_f64().unwrap_or(0.0);
                println!("  {:<8} {:<20} ${:<14.5} ${:<14.5}", mode_name, model, input_cost, total_cost);
            }
            println!();
        }
    }

    Ok(())
}
