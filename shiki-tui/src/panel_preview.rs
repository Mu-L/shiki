use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Focus};
use crate::icons;
use crate::render::{hex_to_color, markdown_to_lines, panel_block};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let focused = app.focus == Focus::Preview;
    let fg = hex_to_color(&app.theme.fg);
    let accent = hex_to_color(&app.theme.accent);
    let muted = hex_to_color(&app.theme.muted);
    let link = hex_to_color(&app.theme.link);

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
        (Some(note), _) => markdown_to_lines(&note.body, fg, accent, muted, link),
        (None, Some(folder)) => folder_preview_lines(app, folder, fg, accent, muted),
        (None, None) => vec![Line::from(ratatui::text::Span::styled(
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

/// What's inside the selected folder, so landing on it (without descending)
/// already shows what you'd find there instead of just a "press enter"
/// hint — a quick peek, same spirit as the note preview itself.
fn folder_preview_lines<'a>(
    app: &App,
    folder: &str,
    fg: ratatui::style::Color,
    accent: ratatui::style::Color,
    muted: ratatui::style::Color,
) -> Vec<Line<'a>> {
    let Some(nb) = app.selected_notebook() else {
        return Vec::new();
    };
    let sub_path = app.notes_relative_path().join(folder);
    let Ok((subfolders, notes)) = nb.list_dir(&sub_path) else {
        return Vec::new();
    };

    if subfolders.is_empty() && notes.is_empty() {
        return vec![Line::from(ratatui::text::Span::styled(
            "Empty folder.".to_string(),
            Style::default().fg(muted).add_modifier(Modifier::ITALIC),
        ))];
    }

    let mut lines = Vec::with_capacity(subfolders.len() + notes.len());
    for name in &subfolders {
        lines.push(Line::from(ratatui::text::Span::styled(
            format!("{}  {name}/", icons::NOTEBOOK),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )));
    }
    for note in &notes {
        lines.push(Line::from(ratatui::text::Span::styled(
            format!("{}  {}", icons::NOTE, note.frontmatter.title),
            Style::default().fg(fg),
        )));
    }
    lines
}
