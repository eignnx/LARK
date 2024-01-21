use lark_vm::cpu;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use super::{utils, App};

impl App {
    pub fn ui(&self, f: &mut Frame) {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(24), // Virtual Terminal
                Constraint::Min(0),     // Command output
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
        for msg in self.cmd_output.iter().rev() {
            match msg {
                CmdMsg::Error(line) => {
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
                CmdMsg::Info(line) => {
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
                CmdMsg::Log(line) => {
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
                CmdMsg::Command(line) => {
                    list_items.push(ListItem::new(Line {
                        spans: vec![
                            Span::styled("> ", Style::default().italic()),
                            Span::styled(line, Style::default().italic()),
                        ],
                        ..Default::default()
                    }));
                }
            }
        }

        f.render_widget(
            List::new(list_items)
                .block(Block::default().borders(Borders::ALL))
                .direction(ratatui::widgets::ListDirection::BottomToTop),
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
}

pub enum CmdMsg {
    Log(String),
    Info(String),
    Error(String),
    Command(String),
}
