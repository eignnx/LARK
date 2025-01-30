use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::mpsc::{Receiver, Sender},
    time::{Duration, Instant},
};

use anyhow::Result;
use crossterm::event::MouseEvent;
use ratatui::prelude::*;

use lark_vm::cpu::{self, instr::Instr, Cpu, MemBlock};
use tui_scrollview::ScrollViewState;

use crate::cli::Opts;

use self::ui::CmdMsg;

mod ui;
mod update;
mod utils;

// App state
pub struct App {
    cpu: Cpu,
    meadowlark_src: Option<PathBuf>,
    lark_src: Option<PathBuf>,
    romfile: Option<PathBuf>,
    vtty_buf: Rc<RefCell<MemBlock<{ cpu::VTTY_BYTES }>>>,

    disassembly: Vec<Instr>,

    cpu_signal_channel: Receiver<lark_vm::cpu::Signal>,
    cpu_interrupt_channel: Sender<lark_vm::cpu::interrupts::Interrupt>,
    cpu_run_till_breakpoint: bool,
    /// The command currently being typed.
    cmd_input: tui_input::Input,
    cmd_input_focus: bool,
    cmd_output: Vec<CmdMsg>,
    cmd_output_scroll: usize,
    cmd_history: Vec<String>,
    cmd_history_idx: usize,

    instr_stopwatch_start: Instant,
    instr_time_delta: Option<Duration>,

    mouse_click: Option<MouseEvent>,
    tab_idx: usize,
    disassembly_scroll_view_state: ScrollViewState,

    should_quit: bool,
}

impl App {
    pub fn new(opts: Opts) -> Self {
        let vtty_buf = Rc::new(RefCell::new(MemBlock::new_zeroed()));
        let cmd_history = Self::load_histfile();
        let (tx, rx) = std::sync::mpsc::channel();
        let (interrupt_tx, interrupt_rx) = std::sync::mpsc::channel();

        let cpu = Cpu::new(Default::default(), vtty_buf.clone(), tx, interrupt_rx);

        let session = Self::load_session();

        let mut app = Self {
            cpu,
            meadowlark_src: opts.meadowlark_src.or(session.meadowlark_src),
            lark_src: opts.lark_src.or(session.lark_src),
            romfile: opts.romfile.or(session.romfile),
            vtty_buf,

            disassembly: Vec::new(),

            cpu_signal_channel: rx,
            cpu_interrupt_channel: interrupt_tx,
            cpu_run_till_breakpoint: false,

            cmd_input: tui_input::Input::default(),
            cmd_input_focus: true,
            cmd_output: Vec::new(),
            cmd_output_scroll: 0,
            cmd_history,
            cmd_history_idx: 0,

            instr_stopwatch_start: Instant::now(),
            instr_time_delta: None,

            mouse_click: None,
            tab_idx: session.tab_idx,
            disassembly_scroll_view_state: ScrollViewState::default(),

            should_quit: false,
        };

        if let Some(romfile) = app.romfile.as_ref() {
            app.load_rom(&romfile.to_owned());
        }

        app
    }

    pub fn run(&mut self) -> Result<()> {
        // setup terminal
        utils::startup()?;

        let result = self.main_loop();

        // teardown terminal before unwrapping Result of app run
        utils::shutdown()?;

        result?;

        Ok(())
    }

    fn main_loop(&mut self) -> Result<()> {
        // ratatui terminal
        let mut t = Terminal::new(CrosstermBackend::new(std::io::stderr()))?;

        loop {
            // application update
            self.update()?;

            // application render
            t.draw(|f| {
                self.ui(f);
            })?;

            // application exit
            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    #[allow(unused)]
    fn cmd_log(&mut self, cmd: impl Into<String>) {
        self.cmd_output.push(CmdMsg::Log(cmd.into()));
    }

    #[allow(unused)]
    fn cmd_info(&mut self, cmd: impl Into<String>) {
        self.cmd_output.push(CmdMsg::Info(cmd.into()));
    }

    #[allow(unused)]
    fn cmd_err(&mut self, cmd: impl Into<String>) {
        self.cmd_output.push(CmdMsg::Error(cmd.into()));
    }

    #[allow(unused)]
    fn log_command(&mut self, cmd: impl Into<String>) {
        self.cmd_output.push(CmdMsg::Command(cmd.into()));
    }

    fn histfile_path() -> PathBuf {
        directories_next::ProjectDirs::from("com", "eignnx", "lark")
            .unwrap()
            .config_dir()
            .join("cmd_history.txt")
    }

    fn load_histfile() -> Vec<String> {
        std::fs::read_to_string(Self::histfile_path())
            .unwrap_or_default()
            .lines()
            .map(|s| s.to_owned())
            .collect()
    }

    fn load_session() -> Session {
        let path = directories_next::ProjectDirs::from("com", "eignnx", "lark")
            .unwrap()
            .config_dir()
            .join("session.ini");

        let s = std::fs::read_to_string(&path).unwrap_or_default();

        Session::deserialize(&s)
    }

    fn save_session(session: &Session) {
        let path = directories_next::ProjectDirs::from("com", "eignnx", "lark")
            .unwrap()
            .config_dir()
            .join("session.ini");

        std::fs::write(&path, session.serialize()).unwrap();
    }
}

struct Session {
    meadowlark_src: Option<PathBuf>,
    lark_src: Option<PathBuf>,
    romfile: Option<PathBuf>,
    tab_idx: usize,
}

impl Session {
    fn serialize(&self) -> String {
        use std::fmt::Write;

        let mut s = String::new();

        if let Some(path) = &self.meadowlark_src {
            writeln!(s, "meadowlark_src = {}", path.display()).unwrap();
        }

        if let Some(path) = &self.lark_src {
            writeln!(s, "lark_src = {}", path.display()).unwrap();
        }

        if let Some(path) = &self.romfile {
            writeln!(s, "romfile = {}", path.display()).unwrap();
        }

        writeln!(s, "tab_idx = {}", self.tab_idx).unwrap();

        s
    }

    fn deserialize(s: &str) -> Self {
        let mut meadowlark_src = None;
        let mut lark_src = None;
        let mut romfile = None;
        let mut tab_idx = 0;

        for line in s.lines() {
            let (key, value) = line.split_once(" = ").unwrap();

            match key {
                "meadowlark_src" => meadowlark_src = Some(PathBuf::from(value)),
                "lark_src" => lark_src = Some(PathBuf::from(value)),
                "romfile" => romfile = Some(PathBuf::from(value)),
                "tab_idx" => tab_idx = value.parse().unwrap_or_default(),
                _ => {}
            }
        }

        Self {
            meadowlark_src,
            lark_src,
            romfile,
            tab_idx,
        }
    }
}

const MAX_HIST_LEN: usize = 512;

impl Drop for App {
    fn drop(&mut self) {
        use std::io::Write;

        let path = Self::histfile_path();

        // Create parent directories if they don't exist
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();

        eprintln!("Writing cmd history to: `{}`", path.display());

        // Open file for writing, create if it doesn't exist
        let mut f = std::fs::File::options()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)
            .unwrap();

        // Limit the entries to the last MAX_HIST_LEN items
        let it = self
            .cmd_history
            .iter()
            .skip(self.cmd_history.len().saturating_sub(MAX_HIST_LEN));

        for line in it {
            writeln!(f, "{}", line).unwrap();
        }

        let session = Session {
            meadowlark_src: self.meadowlark_src.take(),
            lark_src: self.lark_src.take(),
            romfile: self.romfile.take(),
            tab_idx: self.tab_idx,
        };

        Self::save_session(&session);
        utils::shutdown().unwrap();
    }
}
