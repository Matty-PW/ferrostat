use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Gauge, Row, Sparkline, Table},
};

use crate::app::{App, SortBy};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [cpu_area, mem_area, cpu_graph_area, table_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(8),
        Constraint::Min(5),
    ])
    .areas(frame.area());

    let cpu = app.cpu_percent();
    frame.render_widget(
        Gauge::default()
            .block(Block::bordered().title(" CPU "))
            .gauge_style(Style::default().fg(Color::Cyan))
            .percent(cpu.clamp(0.0, 100.0) as u16)
            .label(format!("{cpu:.1}%")),
        cpu_area,
    );

    let mem = app.mem_percent();
    frame.render_widget(
        Gauge::default()
            .block(Block::bordered().title(" Memory "))
            .gauge_style(Style::default().fg(Color::Green))
            .percent(mem.clamp(0.0, 100.0) as u16)
            .label(format!("{mem:.1}%")),
        mem_area,
    );

    // inner width = area width minus the two border columns
    let width = cpu_graph_area.width.saturating_sub(2) as usize;
    let data = app.cpu_history.recent(width);

    frame.render_widget(
        Sparkline::default()
            .block(Block::bordered().title(" CPU history "))
            .style(Style::default().fg(Color::Cyan))
            .max(100)
            .data(&data),
        cpu_graph_area,
    );

    let label = |text: &str, col: SortBy| {
        if app.sort_by != col {
            return text.to_string();
        }
        let arrow = match col {
            SortBy::Pid | SortBy::Name => "▲",
            SortBy::Cpu | SortBy::Memory => "▼",
        };
        format!("{text} {arrow}")
    };

    let header = Row::new([
        label("PID", SortBy::Pid),
        label("Name", SortBy::Name),
        label("CPU%", SortBy::Cpu),
        label("Memory", SortBy::Memory),
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );

    let rows = app.processes.iter().map(|p| {
        Row::new([
            p.pid.to_string(),
            p.name.clone(),
            format!("{:.1}", p.cpu),
            format_bytes(p.memory),
        ])
    });

    let width = [
        Constraint::Length(8),
        Constraint::Min(10),
        Constraint::Length(7),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, width)
        .header(header)
        .block(Block::bordered().title(" Processes (↑↓/jk select, 's' to sort) "))
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(table, table_area, &mut app.table_state);
}

fn format_bytes(bytes: u64) -> String {
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let b = bytes as f64;
    if b >= GIB {
        format!("{:.1} GIB", b / GIB)
    } else {
        format!("{:.1} MIB", b / MIB)
    }
}
