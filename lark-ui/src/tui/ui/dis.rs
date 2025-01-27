use ratatui::{prelude::*, widgets::*};

use crate::tui::App;

impl App {
    pub(super) fn render_disassembly(&mut self, f: &mut Frame, col: Rect) {
        if self.disassembly.is_empty() {
            return;
        }

        let mut byte_idx = lark_vm::cpu::Memory::ROM_START;
        let mut items = Vec::new();

        for instr in self.disassembly.iter() {
            let instr_txt = format!("{instr}");
            let instr_txt = if let Some((op, args)) = instr_txt.split_once('\t') {
                format!("{:<8}{}", op, args)
            } else {
                format!("{}", instr)
            };

            let item = ListItem::new(format!("0x{:04X}    {}", byte_idx, instr_txt));

            if self.cpu.pc == byte_idx {
                items.push(item.style(Style::new().reversed()));
            } else {
                items.push(item);
            }

            byte_idx += instr.instr_size();
        }

        let list =
            List::new(items).block(Block::default().borders(Borders::ALL).title("Disassembly"));
        f.render_widget(list, col);
    }
}
