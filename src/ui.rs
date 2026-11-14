use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Gauge, LineGauge, Padding, Paragraph, Row, Sparkline, Table},
};

use crate::app::{App, Mode, SortBy};
use crate::history::History;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [top_area, mem_area, table_area] = Layout::vertical([
        Constraint::Length(10),
        Constraint::Length(3),
        Constraint::Min(5),
    ])
    .areas(frame.area());

    let [cpu_area, net_area] =
        Layout::horizontal([Constraint::Fill(2), Constraint::Fill(1)]).areas(top_area);

    draw_cpu_panel(frame, cpu_area, app);
    draw_net_panel(frame, net_area, app);

    let mem = app.mem_percent();
    frame.render_widget(
        Gauge::default()
            .block(Block::bordered().title(format!(
                " Memory {} / {}",
                format_bytes(app.used_memory()),
                format_bytes(app.total_memory())
            )))
            .gauge_style(Style::default().fg(Color::Green))
            .percent(mem.clamp(0.0, 100.0) as u16)
            .label(format!("{mem:.1}%")),
        mem_area,
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

    if app.mode == Mode::Help {
        draw_help_popup(frame);
    }
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

fn format_rate(bytes_per_sec: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;

    let b = bytes_per_sec as f64;
    if b >= MIB {
        format!("{:.1} MiB/s", b / MIB)
    } else if b >= KIB {
        format!("{:.1} KiB/s", b / KIB)
    } else {
        format!("{bytes_per_sec} B/s")
    }
}

const CORE_COL_WIDTH: u16 = 20;

fn draw_cpu_panel(frame: &mut Frame, area: Rect, app: &App) {
    // outer border
    let threads = app.per_core.len();
    let counts = match app.physical_cores {
        Some(cores) => format!("{cores}C/{threads}T"),
        None => format!("{threads}T"),
    };
    let block = Block::bordered().title(format!(
        " {} · {counts} · {:.1}% ",
        app.cpu_brand,
        app.cpu_percent()
    ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows_per_col = (inner.height as usize).max(1);
    let cols = app.per_core.len().div_ceil(rows_per_col);
    let cores_width = (cols as u16 * CORE_COL_WIDTH).min(inner.width / 2);

    let [cpu_graph_area, cores_area] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(cores_width)]).areas(inner);

    let data = app.cpu_history.recent(cpu_graph_area.width as usize);

    frame.render_widget(
        Sparkline::default()
            .style(Style::default().fg(Color::Cyan))
            .max(100)
            .data(&data),
        cpu_graph_area,
    );

    if cols == 0 {
        return;
    }

    let col_areas = Layout::horizontal(vec![Constraint::Ratio(1, cols as u32); cols])
        .spacing(1)
        .split(cores_area);

    for (i, history) in app.per_core.iter().enumerate() {
        let col = col_areas[i / rows_per_col];
        let row = (i % rows_per_col) as u16;
        let cell = Rect {
            x: col.x,
            y: col.y + row,
            width: col.width,
            height: 1,
        };

        let percent = history.latest().unwrap_or(0);
        frame.render_widget(
            LineGauge::default()
                .ratio((percent as f64 / 100.0).min(1.0))
                .label(format!("C{:>3}{percent:>3}%", i + 1))
                .filled_style(Style::default().fg(load_color(percent)))
                .filled_symbol("━")
                .unfilled_symbol("━")
                .unfilled_style(Style::default().fg(Color::DarkGray)),
            cell,
        );
    }
}

fn load_color(percent: u64) -> Color {
    match percent {
        0..50 => Color::Green,
        50..80 => Color::Yellow,
        _ => Color::Red,
    }
}

fn draw_net_panel(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::bordered().title(" Network ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [down_area, up_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)]).areas(inner);

    draw_rate_graph(frame, down_area, "↓", &app.net_receive, Color::Green);
    draw_rate_graph(frame, up_area, "↑", &app.net_transmit, Color::Magenta);
}

fn draw_rate_graph(frame: &mut Frame, area: Rect, arrow: &str, history: &History, color: Color) {
    let [label_area, graph_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(area);

    let data = history.recent(graph_area.width as usize);
    let latest = history.latest().unwrap_or(0);
    let peak = data.iter().copied().max().unwrap_or(0);

    frame.render_widget(
        Line::from(format!(
            "{arrow} {} (peak {})",
            format_rate(latest),
            format_rate(peak)
        ))
        .style(Style::default().fg(color)),
        label_area,
    );

    frame.render_widget(
        Sparkline::default()
            .style(Style::default().fg(color))
            .max(peak.max(1))
            .data(&data),
        graph_area,
    );
}

const KEYBINDINGS: &[(&str, &str)] = &[
    ("↑ / k", "Select previous process"),
    ("↓ / j", "Select next process"),
    ("s", "Cycle sort column"),
    ("?", "Toggle this help"),
    ("q / Esc", "Quit"),
];

fn draw_help_popup(frame: &mut Frame) {
    let height = KEYBINDINGS.len() as u16 + 4;
    let area = centered_rect(44, height, frame.area());

    let lines: Vec<Line> = KEYBINDINGS
        .iter()
        .map(|(key, desc)| {
            Line::from(vec![
                Span::styled(
                    format!("{key:>9}  "),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(*desc),
            ])
        })
        .collect();

    let popup = Paragraph::new(lines).block(
        Block::bordered()
            .title(" Help ")
            .title_bottom(Line::from(" Esc to close ").centered())
            .padding(Padding::vertical(1)),
    );

    frame.render_widget(Clear, area);
    frame.render_widget(popup, area);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let [area] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(area);

    let [area] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    area
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_rates() {
        assert_eq!(format_rate(0), "0 B/s");
        assert_eq!(format_rate(512), "512 B/s");
        assert_eq!(format_rate(1536), "1.5 KiB/s");
        assert_eq!(format_rate(5 * 1024 * 1024), "5.0 MiB/s");
    }

    #[test]
    fn centered_rect_is_centered() {
        let area = Rect::new(0, 0, 100, 50);
        assert_eq!(centered_rect(40, 10, area), Rect::new(30, 20, 40, 10));
    }
}
