use std::{
    cmp::Reverse,
    io,
    time::{Duration, Instant},
};

use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    widgets::TableState,
};

use sysinfo::{
    CpuRefreshKind, MINIMUM_CPU_UPDATE_INTERVAL, MemoryRefreshKind, Networks, ProcessRefreshKind,
    ProcessesToUpdate, RefreshKind, System,
};

use crate::history::History;
use crate::ui;

// how often to refresh metrics
const TICK_RATE: Duration = Duration::from_millis(1000);
const HISTORY_LEN: usize = 200;

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Normal,
    Help,
}

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
    pub cpu_history: History,
    pub processes: Vec<ProcessInfo>,
    pub table_state: TableState,
    pub sort_by: SortBy,
    pub per_core: Vec<History>,
    pub cpu_brand: String,
    pub physical_cores: Option<usize>,
    networks: Networks,
    last_net_refresh: Instant,
    net_elapsed: Duration,
    pub net_receive: History,
    pub net_transmit: History,
    pub mode: Mode,
}

impl App {
    pub fn new() -> Self {
        // only load what displays
        let sys = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        )
        .expect("failed to read system info");

        let core_count = sys.cpus().len();
        let per_core = (0..core_count).map(|_| History::new(HISTORY_LEN)).collect();
        let cpu_brand = sys
            .cpus()
            .first()
            .map(|cpu| {
                cpu.brand()
                    .trim()
                    .trim_end_matches(" Processor")
                    .to_string()
            })
            .unwrap_or_default();
        let physical_cores = System::physical_core_count().ok();

        Self {
            sys,
            running: true,
            cpu_history: History::new(HISTORY_LEN),
            processes: Vec::new(),
            table_state: TableState::default().with_selected(Some(0)),
            sort_by: SortBy::Cpu,
            per_core,
            physical_cores,
            cpu_brand,
            networks: Networks::new_with_refreshed_list()
                .expect("oh no, failed to read network interfaces"),
            last_net_refresh: Instant::now(),
            net_elapsed: Duration::ZERO,
            net_receive: History::new(HISTORY_LEN),
            net_transmit: History::new(HISTORY_LEN),
            mode: Mode::Normal,
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
        self.networks.refresh(true);
        let now = Instant::now();
        self.net_elapsed = now.duration_since(self.last_net_refresh);
        self.last_net_refresh = now;
    }

    fn refresh(&mut self) {
        self.refresh_sys();

        let cpu = self.cpu_percent().round() as u64;

        self.cpu_history.push(cpu);

        for (history, cpu) in self.per_core.iter_mut().zip(self.sys.cpus()) {
            history.push(cpu.usage().round() as u64);
        }

        let secs = self.net_elapsed.as_secs_f64();
        if secs > 0.0 {
            let receive: u64 = self.networks.values().map(|n| n.received()).sum();
            let transmit: u64 = self.networks.values().map(|n| n.transmitted()).sum();
            self.net_receive.push((receive as f64 / secs) as u64);
            self.net_transmit.push((transmit as f64 / secs) as u64);
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

    pub fn used_memory(&self) -> u64 {
        self.sys.used_memory()
    }

    pub fn total_memory(&self) -> u64 {
        self.sys.total_memory()
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
            if event::poll(timeout)?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                self.handle_key(key.code);
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
        match self.mode {
            Mode::Normal => self.handle_normal_key(code),
            Mode::Help => self.handle_help_key(code),
        }
    }

    fn handle_normal_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.running = false,
            KeyCode::Char('?') => self.mode = Mode::Help,
            KeyCode::Down | KeyCode::Char('j') => self.select_next(),
            KeyCode::Up | KeyCode::Char('k') => self.select_previous(),
            KeyCode::Char('s') => self.cycle_sort(),
            _ => {}
        }
    }

    fn handle_help_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => self.mode = Mode::Normal,
            _ => {}
        }
    }

    fn sort_processes(&mut self) {
        match self.sort_by {
            SortBy::Cpu => self.processes.sort_by(|a, b| b.cpu.total_cmp(&a.cpu)),
            SortBy::Memory => self.processes.sort_by_key(|p| Reverse(p.memory)),
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
