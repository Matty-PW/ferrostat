use ratatui::{ 
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Gauge, Sparkline, Row, Table},
    Frame,
};

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &App) {
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
        let skip = app.cpu_history.len().saturating_sub(width);
        let data: Vec<u64> = app.cpu_history.iter().skip(skip).copied().collect();

        frame.render_widget(
            Sparkline::default()
                .block(Block::bordered().title(" CPU history "))
                .style(Style::default().fg(Color::Cyan))
                .max(100)
                .data(&data),
            cpu_graph_area,
        );

        let header = Row::new(["PID", "Name", "CPU%", "Memory"])
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

        let rows = app.processes.iter().map(|p| {
            Row::new([
                p.pid.to_string(),
                p.name.clone(),
                format!("{:.1}", p.cpu),
                format_bytes(p.memory),
            ])
        });

        let widths = [
            Constraint::Length(8), 
            Constraint::Min(10),
            Constraint::Length(7),
            Constraint::Length(10),
        ];

        frame.render_widget(
            Table::new(rows, widths)
                .header(header)
                .block(Block::bordered().title(" Processes ")),
            table_area,
        );
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