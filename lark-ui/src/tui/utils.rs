use anyhow::Result;
use crossterm::{
    event::EnableMouseCapture,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

/// helper function to create a centered rect using up certain percentage of the available rect `r`
pub fn centered_inline(desired_width: u16, parent: Rect) -> Rect {
    let total_width = parent.width;
    let padding = total_width.saturating_sub(desired_width) / 2;

    // Cut the given rectangle into three horizontal pieces
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Max(padding),
            Constraint::Max(desired_width),
            Constraint::Max(padding),
        ])
        .split(parent)[1] // Return the middle chunk
}

pub fn startup() -> Result<()> {
    enable_raw_mode()?;
    execute!(std::io::stderr(), EnterAlternateScreen, EnableMouseCapture)?;
    Ok(())
}

pub fn shutdown() -> Result<()> {
    execute!(std::io::stderr(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
