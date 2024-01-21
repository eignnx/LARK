use std::{cell::RefCell, io::Write, path::PathBuf, rc::Rc, sync::mpsc::Receiver};

use anyhow::Result;
use ratatui::prelude::*;

use lark_vm::cpu::{self, Cpu, MemBlock};

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

    cpu_logger: Receiver<lark_vm::cpu::LogMsg>,

    /// The command currently being typed.
    cmd_input: tui_input::Input,
    cmd_output: Vec<CmdMsg>,
    cmd_history: Vec<String>,
    cmd_history_idx: usize,
    should_quit: bool,
}

impl App {
    pub fn new(opts: Opts) -> Self {
        let vtty_buf = Rc::new(RefCell::new(MemBlock::new_zeroed()));
        let cmd_history = Self::load_histfile();
        let (tx, rx) = std::sync::mpsc::channel();

        Self {
            cpu: Cpu::new(Default::default(), vtty_buf.clone(), tx),
            meadowlark_src: opts.meadowlark_src,
            lark_src: opts.lark_src,
            romfile: opts.romfile,
            vtty_buf,
            cpu_logger: rx,
            cmd_input: tui_input::Input::default(),
            cmd_output: Vec::new(),
            cmd_history,
            cmd_history_idx: 0,
            should_quit: false,
        }
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
}

const MAX_HIST_LEN: usize = 512;

impl Drop for App {
    fn drop(&mut self) {
        let path = Self::histfile_path();

        // Create parent directories if they don't exist
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();

        eprintln!("Writing cmd history to: `{}`", path.display());

        // Open file for writing, create if it doesn't exist
        let mut f = std::fs::File::options()
            .create(true)
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
    }
}
