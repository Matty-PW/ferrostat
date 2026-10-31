use std::{ 
    io, 
    time::{Duration, Instant},
};

use ratatui::{ 
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    layout::{Constraint, Layout},
    style::{Color, Style},
    widgets::{Block, Gauge},
    DefaultTerminal, Frame,
};

use sysinfo::{
    CpuRefreshKind, 
    MemoryRefreshKind,
    RefreshKind, 
    System, 
    MINIMUM_CPU_UPDATE_INTERVAL
};

// how often to refresh metrics
const TICK_RATE: Duration = Duration::from_millis(1000);

struct App {
    sys: System,
    running: bool,
}

impl App {
    fn new() -> Self {
        // only load what displays
        let sys = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );
        Self { sys: sys.expect("REASON"), running: true }
    }

    fn refresh(&mut self) {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
    }

    fn cpu_percent(&self) -> f64 {
        f64::from(self.sys.global_cpu_usage())
    }

    fn mem_percent(&self) -> f64 {
        let total = self.sys.total_memory();
        if total == 0 {
            return 0.0;
        }
        self.sys.used_memory() as f64 / total as f64 * 100.0
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        self.sys.refresh_cpu_usage();
        std::thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL);
        self.refresh();

        let mut last_tick = Instant::now();

        while self.running {
            terminal.draw(|frame| self.draw(frame))?;

            // wait for a key but no longer than the time left until the next tick
            let timeout = TICK_RATE.saturating_sub(last_tick.elapsed());
            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key.code);
                    }
                }
            }

            if last_tick.elapsed() >= TICK_RATE {
                self.refresh();
                last_tick = Instant::now();
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.running = false,
            _ => {}
        }
    }

    fn draw(&self, frame: &mut Frame) {
        let [cpu_area, mem_area] =
            Layout::vertical([Constraint::Length(3), Constraint::Length(3)]).areas(frame.area());

        let cpu = self.cpu_percent();
        frame.render_widget(    
            Gauge::default()
                .block(Block::bordered().title(" CPU "))
                .gauge_style(Style::default().fg(Color::Cyan))
                .percent(cpu.clamp(0.0, 100.0) as u16)
                .label(format!("{cpu:.1}%")),
            cpu_area,
        );

        let mem = self.mem_percent();
        frame.render_widget(    
            Gauge::default()
                .block(Block::bordered().title(" Memory (q to quit) "))
                .gauge_style(Style::default().fg(Color::Green))
                .percent(mem.clamp(0.0, 100.0) as u16)
                .label(format!("{mem:.1}%")),
            mem_area,
        );
    }
}

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = App::new().run(&mut terminal);
    ratatui::restore();
    result
}
