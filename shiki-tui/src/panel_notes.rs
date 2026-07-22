use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{List, ListItem, ListState};
use ratatui::Frame;

use crate::app::{App, Focus};
use crate::icons;
use crate::render::{hex_to_color, panel_block, render_scrollbar};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let focused = app.focus == Focus::Notes;
    let fg = hex_to_color(&app.theme.fg);
    let muted = hex_to_color(&app.theme.muted);
    let total = app.folders.len() + app.notes.len();

    let items: Vec<ListItem> = if total == 0 {
        vec![ListItem::new("  no notes yet — press `a` to create one")
            .style(Style::default().fg(muted))]
    } else {
        app.folders
            .iter()
            .map(|name| {
                ListItem::new(format!("{}  {name}/", icons::NOTEBOOK)).style(
                    Style::default()
                        .fg(hex_to_color(&app.theme.accent))
                        .add_modifier(Modifier::BOLD),
                )
            })
            .chain(app.notes.iter().map(|note| {
                ListItem::new(format!("{}  {}", icons::NOTE, note.frontmatter.title))
                    .style(Style::default().fg(fg))
            }))
            .collect()
    };

    let breadcrumb = app
        .notes_breadcrumb()
        .map(|b| format!(" {b}"))
        .unwrap_or_default();
    let title = format!(
        " {}  Notes{breadcrumb} [{}]  {}  [/] search ",
        icons::NOTE,
        app.notes.len(),
        icons::SEARCH
    );
    let highlight_symbol = format!("{} ", icons::ARROW);
    let list = List::new(items)
        .block(panel_block(Line::from(title), focused, &app.theme))
        .highlight_style(
            Style::default()
                .fg(hex_to_color(&app.theme.accent))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(&highlight_symbol);

    let mut state = ListState::default();
    if total != 0 {
        state.select(Some(app.selected_note));
    }
    frame.render_stateful_widget(list, area, &mut state);
    render_scrollbar(frame, area, total, app.selected_note, &app.theme);
}
