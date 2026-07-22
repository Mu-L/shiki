use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Modal confirmation dialog (e.g. before deleting a note or notebook).
#[derive(Debug, Clone)]
pub struct ConfirmDialog {
    pub message: String,
}

impl ConfirmDialog {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Renders into the exact `area` given — the caller (see `app::draw`) is
    /// responsible for centering/sizing it, so this doesn't recompute its own
    /// centered rect on top of an already-centered one.
    pub fn render(&self, frame: &mut Frame, area: Rect, accent: ratatui::style::Color) {
        let paragraph = Paragraph::new(format!("{}  (y/n)", self.message)).block(
            Block::default()
                .title(" Confirm ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(accent)),
        );
        frame.render_widget(paragraph, area);
    }
}
