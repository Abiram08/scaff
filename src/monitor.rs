use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Row, Table},
    Frame, Terminal,
};

use crate::cache::Cache;
use crate::config::ScaffConfig;
use crate::rate_limiter::{Mode, RateLimiter};
use crate::token_tracker::TokenTracker;

fn ui(f: &mut Frame) {
    let config = ScaffConfig::load();
    let tracker = TokenTracker::new();
    let mode = Mode::from_str(&config.api_mode);
    let monthly_used = tracker.monthly_tokens_used();
    let mut rl = RateLimiter::with_usage(mode, monthly_used);
    let monthly = tracker.get_monthly_stats();
    let all_time = tracker.get_all_time_stats();
    let cache = Cache::new();
    let cache_status = cache.status();

    // Pre-bind format strings to avoid temporary-dropped-while-borrowed errors
    let mode_upper = config.api_mode.to_uppercase();
    let month_sessions = monthly.session_count.to_string();
    let all_sessions = all_time.total_sessions.to_string();
    let month_tokens = monthly.total_tokens.to_string();
    let all_tokens = all_time.total_tokens.to_string();
    let month_cost = format!("${:.2}", monthly.total_cost);
    let all_cost = format!("${:.2}", all_time.total_cost);
    let cache_info = format!("{} entries ({} KB)", cache_status.entries, cache_status.size_bytes / 1024);
    let budget_pct = rl.budget_percent_used();
    let remaining = rl.remaining_budget().to_string();
    let util_pct = rl.utilization_percent();
    let daily_burn = monthly.daily_burn.to_string();
    let projected_monthly = monthly.projected_monthly.to_string();
    let limit_str = rl.config.monthly_tokens.to_string();
    let rate_limit_str = format!("{} req/min", rl.config.rate_limit_per_minute);
    let util_pct_str = format!("{:.0}%", util_pct);
    let monthly_limit_str = format!("{} tokens", rl.config.monthly_tokens);
    let monthly_cost_str = format!("${:.2}/mo", rl.config.monthly_cost);
    let used_str = format!("{} tokens ({:.0}%)", monthly_used, budget_pct);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.size());

    // Header
    let header = Paragraph::new(Line::from(vec![
        Span::styled(" scaff monitor ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(format!("v{}", env!("CARGO_PKG_VERSION")), Style::default().fg(Color::DarkGray)),
        Span::raw("  |  "),
        Span::styled(
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            Style::default().fg(Color::DarkGray),
        ),
    ]))
    .block(Block::default().borders(Borders::ALL).style(Style::default().fg(Color::Cyan)));
    f.render_widget(header, chunks[0]);

    // Budget gauge
    let gauge_title = format!("Budget — {} mode", mode_upper);
    let gauge_color = if budget_pct >= 90.0 {
        Color::Red
    } else if budget_pct >= 75.0 {
        Color::Yellow
    } else if budget_pct >= 50.0 {
        Color::Green
    } else {
        Color::Cyan
    };
    let gauge = Gauge::default()
        .block(Block::default().title(format!(" {gauge_title} ")).borders(Borders::ALL))
        .gauge_style(Style::default().fg(gauge_color).bg(Color::DarkGray))
        .percent(budget_pct as u16)
        .label(format!("{:.0}%", budget_pct));
    f.render_widget(gauge, chunks[1]);

    // Burn rate line
    let pct_burn = if rl.config.monthly_tokens > 0 {
        (monthly.projected_monthly as f64 / rl.config.monthly_tokens as f64) * 100.0
    } else {
        0.0
    };
    let trend_style = if pct_burn > 100.0 {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else if pct_burn > 80.0 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Green)
    };
    let trend_text = if pct_burn > 100.0 { "↑ OVER BUDGET" } else if pct_burn > 80.0 { "↑ Near limit" } else { "✓ On track" };
    let burn = Paragraph::new(Line::from(vec![
        Span::raw(format!("Daily: {} tok/day  ", daily_burn)),
        Span::raw(format!("Projected: {} / {} tok  ", projected_monthly, limit_str)),
        Span::styled(trend_text, trend_style),
    ]))
    .block(Block::default().borders(Borders::ALL).title(" Burn Rate "));
    f.render_widget(burn, chunks[2]);

    // Body
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(33), Constraint::Percentage(34), Constraint::Percentage(33)])
        .split(chunks[3]);

    // Rate Limiter panel
    let rate_rows = vec![
        Row::new(vec!["Mode", &mode_upper]),
        Row::new(vec!["Bucket Util", &util_pct_str]),
        Row::new(vec!["Rate Limit", &rate_limit_str]),
    ];
    let rate_widths = [Constraint::Length(15), Constraint::Min(20)];
    let rate_table = Table::new(rate_rows, rate_widths)
        .block(Block::default().title(" Rate Limiter ").borders(Borders::ALL));
    f.render_widget(rate_table, body_chunks[0]);

    // Session + Budget panel
    let budget_rows = vec![
        Row::new(vec!["Monthly Limit", &monthly_limit_str]),
        Row::new(vec!["Monthly Cost", &monthly_cost_str]),
        Row::new(vec!["Used", &used_str]),
        Row::new(vec!["Remaining", &remaining]),
        Row::new(vec!["Sessions (month)", &month_sessions]),
        Row::new(vec!["Sessions (all)", &all_sessions]),
    ];
    let budget_widths = [Constraint::Length(18), Constraint::Min(20)];
    let budget_table = Table::new(budget_rows, budget_widths)
        .block(Block::default().title(" Budget & Sessions ").borders(Borders::ALL));
    f.render_widget(budget_table, body_chunks[1]);

    // Token Usage + System panel
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(body_chunks[2]);

    let usage_rows = vec![
        Row::new(vec!["Metric", "This Month", "All-Time"]),
        Row::new(vec!["Tokens", &month_tokens, &all_tokens]),
        Row::new(vec!["Cost", &month_cost, &all_cost]),
        Row::new(vec!["Daily Burn", &daily_burn, "-"]),
    ];
    let usage_widths = [Constraint::Length(12), Constraint::Length(14), Constraint::Length(14)];
    let usage_table = Table::new(usage_rows, usage_widths)
        .block(Block::default().title(" Token Usage ").borders(Borders::ALL));
    f.render_widget(usage_table, right_chunks[0]);

    let sys_rows = vec![
        Row::new(vec!["Model", &config.model]),
        Row::new(vec!["Provider", &config.default_provider]),
        Row::new(vec!["Output Dir", &config.output_dir]),
        Row::new(vec!["Cache", &cache_info]),
    ];
    let sys_widths = [Constraint::Length(15), Constraint::Min(25)];
    let sys_table = Table::new(sys_rows, sys_widths)
        .block(Block::default().title(" System ").borders(Borders::ALL));
    f.render_widget(sys_table, right_chunks[1]);

    // Footer
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" Press 'q' to quit | Refreshing every 2s ", Style::default().fg(Color::DarkGray)),
    ]))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[4]);
}

pub fn run_dashboard(refresh_secs: u64, once: bool) -> Result<(), String> {
    if once {
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen).map_err(|e| format!("Terminal error: {e}"))?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).map_err(|e| format!("Terminal error: {e}"))?;
        terminal.draw(|f| ui(f)).map_err(|e| format!("Draw error: {e}"))?;
        std::thread::sleep(Duration::from_secs(refresh_secs));
        let mut stdout = io::stdout();
        execute!(stdout, LeaveAlternateScreen).map_err(|e| format!("Terminal error: {e}"))?;
        return Ok(());
    }

    enable_raw_mode().map_err(|e| format!("Raw mode error: {e}"))?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .map_err(|e| format!("Terminal error: {e}"))?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|e| format!("Terminal error: {e}"))?;

    let res = run_live(&mut terminal, refresh_secs);

    disable_raw_mode().map_err(|e| format!("Raw mode error: {e}"))?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .map_err(|e| format!("Terminal error: {e}"))?;
    terminal.show_cursor().map_err(|e| format!("Cursor error: {e}"))?;

    res
}

fn run_live(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, refresh_secs: u64) -> Result<(), String> {
    loop {
        terminal.draw(|f| ui(f)).map_err(|e| format!("Draw error: {e}"))?;

        if event::poll(Duration::from_secs(refresh_secs)).map_err(|e| format!("Event poll error: {e}"))? {
            if let Event::Key(key) = event::read().map_err(|e| format!("Event read error: {e}"))? {
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }
    }
    Ok(())
}
