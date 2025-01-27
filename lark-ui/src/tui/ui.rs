use lark_vm::cpu::{self, ArgStyle};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListDirection, ListItem, Paragraph, Wrap},
};

use super::{utils, App};

impl App {
    pub fn ui(&self, f: &mut Frame) {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(80), // Virtual Terminal
                Constraint::Min(0),  // Side panel (registers, etc)
            ])
            .split(f.size());

        let vtty_cmd_input = cols[0];
        let side_panel = cols[1];

        self.render_vtty_cmd_input(f, vtty_cmd_input);
        self.render_side_panel(f, side_panel);
    }

    fn render_vtty_cmd_input(&self, f: &mut Frame, col: Rect) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(cpu::VTTY_ROWS as u16 + 2), // Vtty height + borders
                Constraint::Min(3),                            // Cmd output height
                Constraint::Length(3),                         // Cmd input height
            ])
            .split(col);

        let vtty_inner_layout = layout[0];
        let cmd_output_row = layout[1];
        let cmd_input_row = layout[2];

        self.render_vtty(f, vtty_inner_layout);
        self.render_cmd_output(f, cmd_output_row);
        self.render_cmd_input(cmd_input_row, f);
    }

    /// Side panel contains the registers display (and in the future other info that can be
    /// tabbed to).
    fn render_side_panel(&self, f: &mut Frame, side_panel: Rect) {
        // Split into lower and upper sections, upper section is for registers, lower is smaller and is for info:
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(15 + 2 + 1 + 2), Constraint::Min(3)])
            .split(side_panel);

        let regs_pane = layout[0];
        let info_pane = layout[1];

        self.render_registers(f, regs_pane);
        self.render_info_pane(f, info_pane);
    }

    fn render_registers(&self, f: &mut Frame<'_>, side_panel: Rect) {
        let reg_lines = self
            .cpu
            .regs
            .iter()
            .map(|(reg, value)| {
                let color = if reg.is_ret_related() {
                    Color::Yellow
                } else if reg.is_argument() {
                    Color::Red
                } else if reg.is_temporary() {
                    Color::Green
                } else if reg.is_saved() {
                    Color::Blue
                } else if reg.is_kernel() {
                    Color::Magenta
                } else {
                    Color::White
                };
                let style = Style::new().fg(color);
                let idx = reg as u8;
                use std::fmt::Write;
                let mut text = String::with_capacity(30);
                write!(text, "${:<2}", idx).unwrap();
                write!(text, " {reg}:").unwrap();
                write!(text, " 0x{:04x}", value.as_u16()).unwrap();
                write!(text, ", {:5}u", value.as_u16()).unwrap();
                write!(text, ", {:+5}", value.as_i16()).unwrap();
                match char::try_from(value.as_u16() as u32) {
                    Ok(ch) if ch.is_ascii_graphic() || ch.is_ascii_whitespace() => {
                        write!(text, ", {:?}", ch).unwrap()
                    }
                    _ => {}
                };
                ListItem::new(Line::styled(text, style))
            })
            .chain([
                {
                    let unsigned = self.cpu.lo.as_u16();
                    let signed = self.cpu.lo.as_i16();
                    ListItem::new(Line::raw(format!(
                        "    $LO: 0x{:04x}, {:5}u, {:+5}",
                        unsigned, unsigned, signed
                    )))
                },
                {
                    let unsigned = self.cpu.hi.as_u16();
                    let signed = self.cpu.hi.as_i16();
                    ListItem::new(Line::raw(format!(
                        "    $HI: 0x{:04x}, {:5}u, {:+5}",
                        unsigned, unsigned, signed
                    )))
                },
                ListItem::new(Line::raw(format!("    $pc: 0x{:04x}", self.cpu.pc))),
            ]);

        f.render_widget(
            List::new(reg_lines).block(Block::default().borders(Borders::ALL).title("Registers")),
            side_panel,
        );
    }

    fn render_info_pane(&self, f: &mut Frame<'_>, side_panel: Rect) {
        let instr_per_sec = if let Some(duration) = self.instr_time_delta {
            if duration.as_secs_f32() > 0.0 {
                1.0f32 / duration.as_secs_f32()
            } else {
                0.0
            }
        } else {
            0.0
        };
        let text = vec![Line::raw(format!("instr/sec: {instr_per_sec:.0}"))];
        f.render_widget(
            List::new(text).block(Block::default().borders(Borders::ALL).title("Info")),
            side_panel,
        );
    }

    fn render_cmd_input(&self, cmd_input_row: Rect, f: &mut Frame<'_>) {
        if self.cmd_input_focus {
            let width = cmd_input_row.width.max(3) - 3;
            // keep 2 for borders and 1 for cursor
            let scroll = self.cmd_input.visual_scroll(width as usize);
            let input = Paragraph::new(self.cmd_input.value())
                .scroll((0, scroll as u16))
                .block(Block::default().borders(Borders::ALL).title("Input"));
            f.render_widget(input, cmd_input_row);
            // Make the cursor visible and ask tui-rs to put it at the specified coordinates after rendering
            f.set_cursor(
                // Put cursor past the end of the input text
                cmd_input_row.x
                    + ((self.cmd_input.visual_cursor()).max(scroll) - scroll) as u16
                    + 1,
                // Move one line down, from the border to the input line
                cmd_input_row.y + 1,
            )
        }
    }

    fn output_cpu_log<'a>(&self, items: &mut Vec<ListItem<'a>>, msg: &'a cpu::LogMsg) {
        match msg {
            cpu::LogMsg::Error(e) => {
                items.push(ListItem::new(Line {
                    spans: vec![Span::raw("CPU ERROR: ").red().bold(), Span::raw(e).red()],
                    ..Default::default()
                }));
            }
            cpu::LogMsg::Instr { size, name, args } => {
                let mut item = vec![];
                item.push(Span::raw(format!("|{}|\t", size)));
                item.push(Span::raw(name).yellow().bold());
                item.push("\t".into());
                for (style, arg) in args {
                    match style {
                        Some(ArgStyle::Reg) => {
                            item.push(" ".into());
                            item.push(Span::raw(arg).magenta());
                        }
                        Some(ArgStyle::Imm) => {
                            item.push(" ".into());
                            item.push(Span::raw(arg).green());
                        }
                        None => {
                            item.push(" ".into());
                            item.push(Span::raw(arg));
                        }
                    }
                }
                items.push(ListItem::new(Line {
                    spans: item,
                    ..Default::default()
                }));
            }

            cpu::LogMsg::DebugPuts { addr, value } => {
                items.push(ListItem::new(Line {
                    spans: vec![
                        Span::raw("DEBUG PUTS ").bold(),
                        Span::raw(format!("0x{:04x}", addr)).cyan(),
                        Span::raw(": "),
                        Span::raw(format!("{:?}", value)).green(),
                    ],
                    ..Default::default()
                }));
            }

            cpu::LogMsg::MmioRead { addr, value } => {
                items.push(ListItem::new(Line {
                    spans: vec![
                        Span::raw("MMIO["),
                        Span::raw(format!("0x{:04x}", addr)).cyan(),
                        Span::raw("] -> "),
                        Span::raw(format!("{:?}", value)).green(),
                    ],
                    ..Default::default()
                }));
            }

            cpu::LogMsg::MmioWrite { addr, value } => {
                items.push(ListItem::new(Line {
                    spans: vec![
                        Span::raw("MMIO["),
                        Span::raw(format!("0x{:04x}", addr)).cyan(),
                        Span::raw("] <- "),
                        Span::raw(format!("{:?}", value)).green(),
                    ],
                    ..Default::default()
                }));
            }
        }
    }

    fn render_cmd_output(&self, f: &mut Frame<'_>, cmd_output_row: Rect) {
        let mut list_items = Vec::<ListItem>::new();

        for msg in self.cmd_output.iter().rev() {
            match msg {
                CmdMsg::Error(lines) => {
                    for line in lines.lines() {
                        list_items.push(ListItem::new(Line {
                            spans: vec![
                                Span::styled(
                                    "ERROR: ",
                                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(line, Style::default().fg(Color::Red)),
                            ],
                            ..Default::default()
                        }));
                    }
                }
                CmdMsg::Info(lines) => {
                    for line in lines.lines() {
                        list_items.push(ListItem::new(Line {
                            spans: vec![
                                Span::styled(
                                    "INFO: ",
                                    Style::default()
                                        .fg(Color::Yellow)
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(line, Style::default().fg(Color::Yellow)),
                            ],
                            ..Default::default()
                        }));
                    }
                }
                CmdMsg::Log(lines) => {
                    for line in lines.lines() {
                        list_items.push(ListItem::new(Line {
                            spans: vec![
                                Span::styled(
                                    "LOG: ",
                                    Style::default()
                                        .fg(Color::Cyan)
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::raw(line),
                            ],
                            ..Default::default()
                        }));
                    }
                }
                CmdMsg::Command(lines) => {
                    for line in lines.lines() {
                        list_items.push(ListItem::new(Line {
                            spans: vec![
                                Span::styled("> ", Style::default().italic()),
                                Span::styled(line, Style::default().italic()),
                            ],
                            ..Default::default()
                        }));
                    }
                }
                CmdMsg::CpuMsg(cpu_msg) => self.output_cpu_log(&mut list_items, cpu_msg),
            }
        }

        let nitems = list_items.len();
        let window_height = cmd_output_row.height as usize;
        let items_to_show = list_items
            .into_iter()
            .skip(self.cmd_output_scroll)
            .take(window_height.min(nitems));

        f.render_widget(
            List::new(items_to_show)
                .block(Block::default().borders(Borders::ALL))
                .direction(ListDirection::BottomToTop),
            cmd_output_row,
        );
    }

    fn render_vtty(&self, f: &mut Frame, row: Rect) {
        let mut lines = Vec::<Line>::new();

        // Interpret vtty buffer as a 2D array of rows of text with maximum 80
        // bytes each.
        for row in 0..cpu::VTTY_ROWS {
            let buf = self.vtty_buf.borrow();
            let row = {
                let row_start = row * cpu::VTTY_COLS;
                let row_end = (row + 1) * cpu::VTTY_COLS;
                &buf.mem[row_start..row_end]
            };
            // Trim everything after the first 0 byte.
            let row_end = row.iter().position(|&b| b == 0).unwrap_or(0);
            // Convert to String, dropping any non-UTF-8 bytes.
            let s = String::from_utf8_lossy(&row[..row_end]).to_string();
            lines.push(Line::raw(s));
        }

        let rect = utils::centered_inline(80, row);
        let w = rect.width;
        let h = rect.height;

        f.render_widget(
            Paragraph::new(lines).wrap(Wrap { trim: false }).block(
                Block::default()
                    .title(format!("Virtual Terminal ({w}x{h})", h = h - 2))
                    .borders(Borders::ALL),
            ),
            row,
        );
    }
}

pub enum CmdMsg {
    Log(String),
    Info(String),
    Error(String),
    Command(String),
    CpuMsg(cpu::LogMsg),
}
