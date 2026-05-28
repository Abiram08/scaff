"""Example agent descriptions and templates."""

EXAMPLE_AGENTS = {
    "email-summarizer": {
        "description": "Summarize emails and flag urgent ones for immediate attention",
        "command": 'scaff create "summarize my emails and flag urgent ones"',
        "tools": ["fetch_emails", "send_alert", "mark_important"],
        "dependencies": ["imaplib2", "email-validator"],
    },
    "github-labeler": {
        "description": "Monitor GitHub issues and auto-label them by priority",
        "command": 'scaff create "monitor my github issues and auto-label them by priority"',
        "tools": ["fetch_issues", "apply_label", "add_comment"],
        "dependencies": ["PyGithub", "requests"],
    },
    "code-analyzer": {
        "description": "Analyze code for security issues and generate reports",
        "command": 'scaff create "analyze code for security issues"',
        "tools": ["scan_code", "generate_report", "detect_vulnerabilities"],
        "dependencies": ["bandit", "pylint"],
    },
    "weather-bot": {
        "description": "Fetch weather data and format it nicely",
        "command": 'scaff create "fetch weather data and format it nicely"',
        "tools": ["get_weather", "format_forecast", "send_notification"],
        "dependencies": ["requests", "aiohttp"],
    },
    "news-curator": {
        "description": "Aggregate news from multiple sources and categorize them",
        "command": 'scaff create "aggregate news from multiple sources and categorize"',
        "tools": ["fetch_news", "categorize", "summarize"],
        "dependencies": ["feedparser", "requests"],
    },
    "slack-assistant": {
        "description": "Manage Slack channels, respond to messages, and track conversations",
        "command": 'scaff create "manage slack channels and respond to messages"',
        "tools": ["send_message", "create_thread", "get_reactions"],
        "dependencies": ["slack-sdk"],
    },
}


def get_example_list() -> str:
    """Return formatted list of example agents."""
    output = ""
    
    for name, info in EXAMPLE_AGENTS.items():
        output += f"  {name.upper()}\n"
        output += f"    Description: {info['description']}\n"
        output += f"    Command: {info['command']}\n"
        output += f"    Tools: {', '.join(info['tools'])}\n"
        output += f"    Dependencies: {', '.join(info['dependencies'])}\n\n"
    
    return output


def get_example_command(agent_name: str) -> str:
    """Get the command for an example agent."""
    agent = EXAMPLE_AGENTS.get(agent_name.lower())
    if agent:
        return agent["command"]
    return None
