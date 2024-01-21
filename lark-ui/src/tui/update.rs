use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use lark_vm::cpu::{Cpu, MemBlock};
use tui_input::backend::crossterm::EventHandler;

use super::App;

impl App {
    // App update function
    pub fn update(&mut self) -> Result<()> {
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('c' | 'd')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            self.should_quit = true
                        }
                        KeyCode::Esc => {
                            self.should_quit = true;
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
        Ok(())
    }

    fn get_history_cmd(&self, idx: usize) -> String {
        self.cmd_history
            .get(self.cmd_history.len() - idx)
            .map(|s| s.to_owned())
            .unwrap_or_default()
    }

    pub fn do_cmd(&mut self, cmd: &str) {
        self.cmd_history.push(cmd.to_owned());
        self.cmd_history_idx = 0;
        self.log_command(cmd);

        match cmd.split_ascii_whitespace().collect::<Vec<_>>().as_slice() {
            ["load" | "l", path] => {
                match PathBuf::from(path)
                    .extension()
                    .map(|ext| ext.to_str().unwrap())
                {
                    // Some("meadowlark") => self.load_meadowlark(path),
                    // Some("lark") => self.load_asm(path),
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
                todo!()
            }
            ["run"] => {
                self.cpu.run();
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
                self.cmd_info("Available commands:".to_string());
                self.cmd_info("  - load <path>".to_string());
                self.cmd_info("  - clearhist".to_string());
                self.cmd_info("  - reset".to_string());
                self.cmd_info("  - run".to_string());
                self.cmd_info("  - hexdump".to_string());
                self.cmd_info("  - help".to_string());
                self.cmd_info("  - quit".to_string());
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
        self.romfile = Some(path);

        // TODO: impl Cpu::reset and Cpu::load_rom
        self.cpu = Cpu::new(rom, self.vtty_buf.clone());
        self.clear_vtty();

        self.cmd_info(format!(
            "Loaded ROM: {}",
            self.romfile.as_ref().unwrap().display()
        ));
    }

    fn clear_vtty(&mut self) {
        let mut vtty_buf = self.vtty_buf.borrow_mut();
        vtty_buf.mem.fill(0);
    }
}
