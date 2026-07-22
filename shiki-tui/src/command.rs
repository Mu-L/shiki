use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem};
use ratatui::Frame;

use shiki_core::{Note, SearchEngine};

use crate::input::InputBox;

/// Global command palette / fuzzy finder over the notes in the current notebook.
#[derive(Default)]
pub struct CommandPalette {
    pub input: InputBox,
    pub engine: SearchEngine,
    pub selected: usize,
}

impl CommandPalette {
    pub fn matches<'a>(&mut self, notes: &'a [Note]) -> Vec<&'a Note> {
        self.engine
            .search(&self.input.value, notes)
            .into_iter()
            .map(|hit| &notes[hit.index])
            .collect()
    }

    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        notes: &[Note],
        accent: ratatui::style::Color,
        fg: ratatui::style::Color,
    ) {
        let matches = self.matches(notes);
        let items: Vec<ListItem> = matches
            .iter()
            .map(|n| ListItem::new(n.frontmatter.title.clone()))
            .collect();

        let block = Block::default()
            .title(format!("/ {}", self.input.value))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent));

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().fg(fg).add_modifier(Modifier::BOLD));

        frame.render_widget(list, area);
    }
}
