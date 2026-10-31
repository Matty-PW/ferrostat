use ratatui::{ 
    layout::{Constraint, Layout},
    style::{Color, Style},
    widgets::{Block, Gauge, Sparkline},
    Frame,
};

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &App) {
        let [cpu_area, mem_area, cpu_graph_area] = Layout::vertical([
            Constraint::Length(3), 
            Constraint::Length(3),
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
    }