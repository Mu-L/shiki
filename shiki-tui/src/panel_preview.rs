use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Focus};
use crate::icons;
use crate::render::{borrow_lines, hex_to_color, panel_block};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let focused = app.focus == Focus::Preview;
    let muted = hex_to_color(&app.theme.muted);

    let title: Line = match (app.selected_note(), app.selected_folder()) {
        (Some(n), _) => Line::from(vec![
            Span::raw(format!(" {}  {}  ", icons::EYE, n.frontmatter.title)),
            Span::styled(
                format!("({}) ", n.frontmatter.date.format("%Y-%m-%d")),
                Style::default().fg(muted),
            ),
        ]),
        (None, Some(folder)) => Line::from(format!(" {}  {folder}/ ", icons::NOTEBOOK)),
        (None, None) => Line::from(format!(" {}  Preview ", icons::EYE)),
    };
    let block = panel_block(title, focused, &app.theme);

    let lines = match (app.selected_note(), app.selected_folder()) {
        // Both branches read from an `App`-level cache
        // (`note_preview_lines`/`folder_preview_lines`) that only re-does
        // the real work — `markdown_to_lines`'s full pass over the note
        // body, or `format_folder_entries`'s per-row `format!` calls —
        // when the selection or theme colors actually changed, not on
        // every one of these render calls.
        (Some(_), _) => borrow_lines(app.note_preview_lines().unwrap_or(&[])),
        (None, Some(_)) => borrow_lines(app.folder_preview_lines().unwrap_or(&[])),
        (None, None) => vec![Line::from(Span::styled(
            "No notes yet in this notebook — press `a` to create one.",
            Style::default().fg(muted).add_modifier(Modifier::ITALIC),
        ))],
    };

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.preview_scroll, 0));
    frame.render_widget(paragraph, area);
}

/// What's inside a selected-but-not-entered folder, so landing on it already
/// shows what you'd find there instead of just a "press enter" hint — a
/// quick peek, same spirit as the note preview itself. Pure formatting, no
/// `App` access — called from `App::refresh_folder_preview_cache` to build
/// the cached lines, not from `render` directly (see there for why).
pub(crate) fn format_folder_entries(
    subfolders: &[String],
    note_titles: &[String],
    fg: Color,
    accent: Color,
    muted: Color,
) -> Vec<Line<'static>> {
    if subfolders.is_empty() && note_titles.is_empty() {
        return vec![Line::from(Span::styled(
            "Empty folder.".to_string(),
            Style::default().fg(muted).add_modifier(Modifier::ITALIC),
        ))];
    }

    let mut lines = Vec::with_capacity(subfolders.len() + note_titles.len());
    for name in subfolders {
        lines.push(Line::from(Span::styled(
            format!("{}  {name}/", icons::NOTEBOOK),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )));
    }
    for title in note_titles {
        lines.push(Line::from(Span::styled(
            format!("{}  {title}", icons::NOTE),
            Style::default().fg(fg),
        )));
    }
    lines
}
