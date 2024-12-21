use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use tui_input::backend::crossterm::EventHandler;

use lark_vm::cpu::{MemBlock, Signal};

use super::{ui::CmdMsg, App};

impl App {
    // App update function
    pub fn update(&mut self) -> Result<()> {
        if self.cpu_run_till_breakpoint {
            self.cpu.step().unwrap_or_else(|e| {
                self.cmd_err(format!("CPU Error: {:?}", e));
            });
        }

        let ui_delay = if self.cpu_run_till_breakpoint { 0 } else { 50 };

        if event::poll(std::time::Duration::from_millis(ui_delay))? {
            let e = event::read()?;
            if let Event::Mouse(m) = e {
                match m.kind {
                    event::MouseEventKind::ScrollDown => {
                        self.cmd_output_scroll = self.cmd_output_scroll.saturating_sub(1);
                    }
                    event::MouseEventKind::ScrollUp => {
                        self.cmd_output_scroll =
                            (self.cmd_output_scroll + 1).min(self.cmd_output.len());
                    }
                    _ => {}
                }
            }

            if let Event::Key(key) = e {
                if key.kind == event::KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('c' | 'd')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            self.should_quit = true
                        }
                        KeyCode::Esc if self.cpu_run_till_breakpoint => {
                            self.cmd_log("CPU halted.".to_string());
                            self.cpu_run_till_breakpoint = false;
                        }
                        KeyCode::Esc => {
                            self.cmd_input.reset();
                        }
                        KeyCode::Up => {
                            self.cmd_history_idx =
                                (self.cmd_history_idx + 1) % self.cmd_history.len();
                            self.cmd_input = tui_input::Input::default()
                                .with_value(self.get_history_cmd(self.cmd_history_idx));
                        }
                        KeyCode::Down => {
                            self.cmd_history_idx = self.cmd_history_idx.saturating_sub(1);
                            self.cmd_input = tui_input::Input::default()
                                .with_value(self.get_history_cmd(self.cmd_history_idx));
                        }
                        KeyCode::Enter => {
                            let cmd = self.cmd_input.value().to_owned();
                            self.cmd_input.reset();
                            self.do_cmd(&cmd);
                        }
                        _ => {
                            self.cmd_input.handle_event(&Event::Key(key));
                        }
                    }
                }
            }
        }

        // Alloc a vec so `self` isn't borrowed immutably. We want to mutate
        // `self` in the match statement below.
        let signals = self.cpu_signal_channel.try_iter().collect::<Vec<_>>();

        for signal in signals {
            match signal {
                Signal::Log(msg) => {
                    self.cmd_output.push(CmdMsg::CpuMsg(msg));
                }
                Signal::Halt => {
                    self.cmd_log("CPU halted.".to_string());
                    self.cpu_run_till_breakpoint = false;
                }
                Signal::Breakpoint => {
                    self.cmd_log(format!("BREAKPOINT at pc=0x{:04x}", self.cpu.pc));
                    self.cpu_run_till_breakpoint = false;
                }
                Signal::IllegalInstr => {
                    self.cmd_err(format!("Illegal instruction at pc=0x{:04x}", self.cpu.pc));
                    self.cpu_run_till_breakpoint = false;
                }
            }
        }

        Ok(())
    }

    fn get_history_cmd(&self, idx: usize) -> String {
        self.cmd_history
            .get(self.cmd_history.len() - idx)
            .map(|s| s.to_owned())
            .unwrap_or_default()
    }

    pub fn do_cmd(&mut self, cmd: &str) {
        // Don't save duplicate commands.
        if self.cmd_history.last().is_some_and(|s| s != cmd) {
            self.cmd_history.push(cmd.to_owned());
            self.cmd_history_idx = 0;
        }

        self.log_command(cmd);
        // Scroll to bottom of output.
        self.cmd_output_scroll = 0;

        match cmd.split_ascii_whitespace().collect::<Vec<_>>().as_slice() {
            ["load" | "l", path] => {
                match PathBuf::from(path)
                    .extension()
                    .map(|ext| ext.to_str().unwrap())
                {
                    Some("meadowlark" | "meadow") => self.load_meadowlark(path),
                    Some("lark" | "asm") => self.load_asm(path),
                    Some("bin" | "rom") => self.load_rom(path),
                    _ => {
                        self.cmd_err(format!("Unknown file extension: {}", path));
                        self.cmd_info("  - Supported extensions: .bin, .rom".to_string());
                    }
                }
            }
            ["clearhist"] => {
                self.cmd_history.clear();
            }
            // Reset the CPU and clear the virtual terminal.
            ["reset"] => {
                self.reset_cpu();
            }
            ["run"] => {
                self.cmd_log(format!(
                    "Running `{}`...",
                    self.romfile
                        .as_ref()
                        .or(self.lark_src.as_ref())
                        .or(self.meadowlark_src.as_ref())
                        .map(|p| p.display().to_string())
                        .unwrap_or("<unknown source file>".to_string())
                ));
                self.cpu_run_till_breakpoint = true;
            }
            ["step" | "s"] => {
                self.cmd_log("Stepping...".to_string());
                self.cpu.step().unwrap_or_else(|e| {
                    self.cmd_err(format!("CPU Error: {:?}", e));
                });
            }
            ["hexdump" | "x"] => {
                self.cmd_info("Hexdump of ROM:".to_string());
                let mut line = String::new();
                let rom = self.cpu.mem.rom.mem.clone();
                for (i, b) in rom.iter().enumerate() {
                    line.push_str(&format!("{:02X} ", b));
                    if i % 16 == 15 {
                        self.cmd_info(line.clone());
                        line.clear();
                    }
                }
            }
            ["help" | "h"] => {
                self.cmd_info("Commands:".to_string());
                self.cmd_info("  - load <path> (l)".to_string());
                self.cmd_info("  - clearhist".to_string());
                self.cmd_info("  - reset".to_string());
                self.cmd_info("  - run".to_string());
                self.cmd_info("  - step (s)".to_string());
                self.cmd_info("  - hexdump (x)".to_string());
                self.cmd_info("  - help (h)".to_string());
                self.cmd_info("  - quit (q)".to_string());
            }
            ["quit" | "q"] => {
                self.cmd_history.pop(); // Don't save quit command
                self.should_quit = true;
            }
            _ => {
                self.cmd_err(format!("Unknown command: `{}`", cmd));
            }
        }
    }

    fn load_meadowlark(&mut self, path: &str) {
        let path = PathBuf::from(path);
        match meadowlark::compile(&path, false) {
            Ok(bin_path) => {
                self.load_rom(bin_path.to_str().unwrap());
                self.meadowlark_src = Some(path);
            }
            Err(e) => {
                self.cmd_err(format!("Error compiling Meadowlark file: {}", e));
            }
        }
    }

    fn load_asm(&mut self, path: &str) {
        todo!()
    }

    fn load_rom(&mut self, path: &str) {
        let path = PathBuf::from(path);
        let rom = match std::fs::read(&path) {
            Ok(rom) => rom,
            Err(e) => {
                self.cmd_err(format!("Error reading ROM file: {}", e));
                return;
            }
        };

        let romfile_size = rom.len();

        let rom = match MemBlock::from_vec(rom) {
            Some(rom) => rom,
            None => {
                self.cmd_err(format!("ROM file too large: {}", path.display()));
                self.cmd_info(format!("  - Max ROM size: {}", lark_vm::cpu::ROM_SIZE));
                self.cmd_info(format!("  - Given ROM size: {romfile_size}"));
                return;
            }
        };

        self.reset_cpu();
        self.romfile = Some(path);
        self.cpu.load_rom(rom);

        self.cmd_info(format!(
            "Loaded ROM: {}",
            self.romfile.as_ref().unwrap().display()
        ));
    }

    fn clear_vtty(&mut self) {
        let mut vtty_buf = self.vtty_buf.borrow_mut();
        vtty_buf.mem.fill(0);
    }

    fn reset_cpu(&mut self) {
        self.cpu.reset();
        self.clear_vtty();
    }
}
