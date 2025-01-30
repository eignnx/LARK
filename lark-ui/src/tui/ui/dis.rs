use lark_vm::{
    cpu::{instr::Instr, regs::Reg},
    utils::s16,
};
use ratatui::{layout::Size, prelude::*, widgets::*};
use tui_scrollview::{ScrollView, ScrollViewState};

pub struct DisassemblyView<'a> {
    pub disassembly: &'a [Instr<Reg, s16>],
    pub pc: u16,
}

impl<'a> StatefulWidget for DisassemblyView<'a> {
    type State = ScrollViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
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

            if self.pc == byte_idx {
                items.push(item.style(Style::new().reversed()));
            } else {
                items.push(item);
            }

            byte_idx += instr.instr_size();
        }

        let content_height = items.len() as u16;
        let list = List::new(items).highlight_style(Style::new().reversed());

        let mut scroll_view = ScrollView::new(Size {
            height: content_height,
            width: area.width,
        })
        .horizontal_scrollbar_visibility(tui_scrollview::ScrollbarVisibility::Never);

        scroll_view.render_widget(list, Rect::new(0, 0, area.width, content_height));
        scroll_view.render(area, buf, state);
    }
}
