use std::{
    collections::VecDeque,
    io,
    time::{Duration, Instant},
};

use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    widgets::TableState,
};

use sysinfo::{
    CpuRefreshKind, MINIMUM_CPU_UPDATE_INTERVAL, MemoryRefreshKind, ProcessRefreshKind,
    ProcessesToUpdate, RefreshKind, System,
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

#[derive(Clone, Copy, PartialEq)]
pub enum SortBy {
    Cpu,
    Memory,
    Pid,
    Name,
}

impl SortBy {
    fn next(self) -> Self {
        match self {
            SortBy::Cpu => SortBy::Memory,
            SortBy::Memory => SortBy::Pid,
            SortBy::Pid => SortBy::Name,
            SortBy::Name => SortBy::Cpu,
        }
    }
}

pub struct App {
    sys: System,
    running: bool,
    pub cpu_history: VecDeque<u64>,
    pub processes: Vec<ProcessInfo>,
    pub table_state: TableState,
    pub sort_by: SortBy,
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
            table_state: TableState::default().with_selected(Some(0)),
            sort_by: SortBy::Cpu,
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

        let selected = self.selected_pid();

        self.processes = self
            .sys
            .processes()
            .values()
            .filter(|p| p.thread_kind().is_none())
            .map(|p| ProcessInfo {
                pid: p.pid().as_u32(),
                name: p.name().to_string_lossy().into_owned(),
                cpu: p.cpu_usage(),
                memory: p.memory(),
            })
            .collect();

        // highest cpu first
        self.sort_processes();
        self.restore_selection(selected);
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

    fn select_next(&mut self) {
        if self.processes.is_empty() {
            return;
        }
        let last = self.processes.len() - 1;
        let i = match self.table_state.selected() {
            Some(i) => (i + 1).min(last),
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    fn select_previous(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => i.saturating_sub(1),
            None => 0,
        };
        self.table_state.select(Some(i))
    }

    fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.running = false,
            KeyCode::Down | KeyCode::Char('j') => self.select_next(),
            KeyCode::Up | KeyCode::Char('k') => self.select_previous(),
            KeyCode::Char('s') => self.cycle_sort(),
            _ => {}
        }
    }

    fn sort_processes(&mut self) {
        match self.sort_by {
            SortBy::Cpu => self.processes.sort_by(|a, b| b.cpu.total_cmp(&a.cpu)),
            SortBy::Memory => self.processes.sort_by(|a, b| b.memory.cmp(&a.memory)),
            SortBy::Pid => self.processes.sort_by_key(|p| p.pid),
            SortBy::Name => self.processes.sort_by_key(|p| p.name.to_lowercase()),
        }
    }

    fn cycle_sort(&mut self) {
        let selected = self.selected_pid();
        self.sort_by = self.sort_by.next();
        self.sort_processes();
        self.restore_selection(selected);
    }

    fn selected_pid(&self) -> Option<u32> {
        self.table_state
            .selected()
            .and_then(|i| self.processes.get(i))
            .map(|p| p.pid)
    }

    fn restore_selection(&mut self, pid: Option<u32>) {
        if self.processes.is_empty() {
            self.table_state.select(None);
            return;
        }
        let last = self.processes.len() - 1;
        let index = pid
            .and_then(|pid| self.processes.iter().position(|p| p.pid == pid))
            .unwrap_or_else(|| self.table_state.selected().unwrap_or(0).min(last));
        self.table_state.select(Some(index));
    }
}
