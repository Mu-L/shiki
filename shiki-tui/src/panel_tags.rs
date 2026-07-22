use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{List, ListItem};
use ratatui::Frame;
use shiki_config::Theme;
use shiki_core::TagIndex;

use crate::icons;
use crate::render::{hex_to_color, panel_block};

/// Tag-filtering popup (key `T`).
pub fn render(frame: &mut Frame, area: Rect, tags: &TagIndex, focused: bool, theme: &Theme) {
    let fg = hex_to_color(&theme.fg);
    let tag_color = hex_to_color(&theme.tag);

    let items: Vec<ListItem> = tags
        .tags()
        .map(|tag| {
            ListItem::new(format!(
                "{} {tag} ({})",
                icons::TAG,
                tags.notes_for(tag).len()
            ))
            .style(Style::default().fg(fg))
        })
        .collect();

    let title = format!(" {}  Tags ", icons::TAG);
    let list = List::new(items)
        .block(panel_block(Line::from(title), focused, theme))
        .highlight_style(Style::default().fg(tag_color));
    frame.render_widget(list, area);
}
