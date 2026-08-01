use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Modal confirmation dialog (e.g. before deleting a note or notebook).
#[derive(Debug, Clone)]
pub struct ConfirmDialog {
    pub message: String,
    hint: String,
}

impl ConfirmDialog {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            hint: "(y/n)".into(),
        }
    }

    /// Same dialog, but with a custom key hint instead of the default
    /// "(y/n)" — for confirmations with more than a plain yes/no choice, e.g.
    /// `App::start_delete_notebook`'s delete-files-vs-keep-files-and-just-untrack
    /// prompt.
    pub fn with_hint(message: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            hint: hint.into(),
        }
    }

    /// Length of the full rendered line (`message` + separator + `hint`) —
    /// `app::draw` sizes the popup off this, not off `message` alone, so a
    /// longer hint (e.g. `with_hint`'s three-way key list) doesn't get
    /// clipped by a popup only wide enough for the plain "(y/n)" case.
    pub fn display_len(&self) -> usize {
        self.message.len() + 2 + self.hint.len()
    }

    /// Renders into the exact `area` given — the caller (see `app::draw`) is
    /// responsible for centering/sizing it, so this doesn't recompute its own
    /// centered rect on top of an already-centered one.
    pub fn render(&self, frame: &mut Frame, area: Rect, accent: ratatui::style::Color) {
        let paragraph = Paragraph::new(format!("{}  {}", self.message, self.hint)).block(
            Block::default()
                .title(" Confirm ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(accent)),
        );
        frame.render_widget(paragraph, area);
    }
}
