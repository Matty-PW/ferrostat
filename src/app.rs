use std::{ 
    collections::VecDeque,
    io, 
    time::{Duration, Instant},
};

use ratatui::{ 
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    DefaultTerminal,
};

use sysinfo::{
    CpuRefreshKind, 
    MemoryRefreshKind,
    ProcessRefreshKind,
    ProcessesToUpdate,
    RefreshKind, 
    System, 
    MINIMUM_CPU_UPDATE_INTERVAL
};

use crate::ui;

// how often to refresh metrics
const TICK_RATE: Duration = Duration::from_millis(1000);
const HISTORY_LEN: usize = 200;

pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    pub memory: u64,
}

pub struct App {
    sys: System,
    running: bool,
    pub cpu_history: VecDeque<u64>,
    pub processes: Vec<ProcessInfo>,
}

impl App {
    pub fn new() -> Self {
        // only load what displays
        let sys = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );
        Self { 
            sys: sys.expect("REASON"),
            running: true,
            cpu_history: VecDeque::with_capacity(HISTORY_LEN),
            processes: Vec::new(),
        }
    }

    fn refresh_sys(&mut self) {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true, // remove processes that have exited
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
    }

    fn refresh(&mut self) {
        self.refresh_sys();

        let cpu = self.cpu_percent().round() as u64;
        self.cpu_history.push_back(cpu);
        if self.cpu_history.len() > HISTORY_LEN {
            self.cpu_history.pop_front();
        }

        self.processes = self
            .sys
            .processes()
            .values()
            .map(|p| ProcessInfo {
                pid: p.pid().as_u32(),
                name: p.name().to_string_lossy().into_owned(),
                cpu: p.cpu_usage(),
                memory: p.memory(),
            })
            .collect();

        // highest cpu first
        self.processes.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
    }

    pub fn cpu_percent(&self) -> f64 {
        f64::from(self.sys.global_cpu_usage())
    }

    pub fn mem_percent(&self) -> f64 {
        let total = self.sys.total_memory();
        if total == 0 {
            return 0.0;
        }
        self.sys.used_memory() as f64 / total as f64 * 100.0
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        self.refresh_sys();
        std::thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL);
        self.refresh();

        let mut last_tick = Instant::now();

        while self.running {
            terminal.draw(|frame| ui::draw(frame, self))?;

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

    
}

