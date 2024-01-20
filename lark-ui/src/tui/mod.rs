use std::{cell::RefCell, rc::Rc};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};
use tui_input::backend::crossterm::EventHandler;

use lark_vm::cpu::{self, Cpu, MemBlock};

use crate::cli::Opts;

mod utils;

// App state
pub struct App {
    cpu: Cpu,
    should_quit: bool,
    vtty_buf: Rc<RefCell<MemBlock<{ cpu::VTTY_BYTES }>>>,
    /// The command currently being typed.
    cmd_input: tui_input::Input,
    cmd_output: Vec<String>,
}

impl App {
    pub fn new(opts: Opts) -> Self {
        let _ = opts;
        let vtty_buf = Rc::new(RefCell::new(MemBlock::new_zeroed()));

        Self {
            cpu: Cpu::new(Default::default(), vtty_buf.clone()),
            should_quit: false,
            vtty_buf,
            cmd_input: tui_input::Input::default(),
            cmd_output: Vec::new(),
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

    fn ui(&self, f: &mut Frame) {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(24), // Virtual Terminal
                Constraint::Min(3),     // Command output
                Constraint::Length(3),  // Input (borders + input line)
            ])
            .split(f.size());

        let vtty_row = rows[0];
        let cmd_output_row = rows[1];
        let cmd_input_row = rows[2];

        self.render_vtty(f, vtty_row);
        self.render_cmd_output(f, cmd_output_row);
        self.render_cmd_input(cmd_input_row, f);
    }

    fn render_cmd_input(&self, cmd_input_row: Rect, f: &mut Frame<'_>) {
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
            cmd_input_row.x + ((self.cmd_input.visual_cursor()).max(scroll) - scroll) as u16 + 1,
            // Move one line down, from the border to the input line
            cmd_input_row.y + 1,
        )
    }

    fn render_cmd_output(&self, f: &mut Frame<'_>, cmd_output_row: Rect) {
        let mut list_items = Vec::<ListItem>::new();
        for line in &self.cmd_output {
            list_items.push(ListItem::new(Line::from(Span::styled(
                line,
                Style::default().fg(Color::Yellow),
            ))));
        }
        f.render_widget(
            List::new(list_items).block(Block::default().borders(Borders::ALL)),
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
                    .title(format!("Virtual Terminal ({w}x{h})"))
                    .borders(Borders::ALL),
            ),
            rect,
        );
    }

    // App update function
    fn update(&mut self) -> Result<()> {
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
                        KeyCode::Enter => {
                            let cmd = self.cmd_input.value();
                            self.cmd_output.push(cmd.into());

                            // TEST
                            {
                                let mut vtty_buf = self.vtty_buf.borrow_mut();
                                for (i, line) in self.cmd_output.iter().enumerate() {
                                    let line = line.as_bytes();
                                    vtty_buf.mem
                                        [i * cpu::VTTY_COLS..i * cpu::VTTY_COLS + line.len()]
                                        .copy_from_slice(line);
                                }
                            }

                            self.cmd_input.reset();
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
}
