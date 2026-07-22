use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
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

    let title = match (app.selected_note(), app.selected_folder()) {
        (Some(n), _) => format!(" {}  {}  [j/k scroll] ", icons::EYE, n.frontmatter.title),
        (None, Some(folder)) => format!(" {}  {folder}/ ", icons::NOTEBOOK),
        (None, None) => format!(" {}  Preview ", icons::EYE),
    };
    let block = panel_block(Line::from(title), focused, &app.theme);

    let lines = match (app.selected_note(), app.selected_folder()) {
        (Some(note), _) => markdown_to_lines(&note.body, fg, accent, muted, link),
        (None, Some(folder)) => vec![Line::from(ratatui::text::Span::styled(
            format!("{folder}/  —  press l / → / enter to open this folder."),
            Style::default().fg(muted).add_modifier(Modifier::ITALIC),
        ))],
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
