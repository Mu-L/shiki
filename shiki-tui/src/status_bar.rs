use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, Focus, Mode};
use crate::icons;
use crate::render::hex_to_color;

/// Only worth announcing when it's not the default — a "NORMAL" label on
/// every frame is noise, but INSERT/EDIT/VISUAL are worth flagging.
fn mode_label(mode: Mode) -> Option<&'static str> {
    match mode {
        Mode::Normal => None,
        Mode::Insert => Some("INSERT"),
        Mode::Edit => Some("EDIT"),
        Mode::Visual => Some("VISUAL"),
    }
}

/// Shortens `text` to at most `max_chars`, marking the cut with `…` — for
/// fitting the status message into whatever footer space is actually left
/// on narrow/small terminals instead of overflowing into the right-aligned
/// help/version text.
fn truncate_to(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    if max_chars <= 1 {
        return "…".to_string();
    }
    let mut truncated: String = text.chars().take(max_chars - 1).collect();
    truncated.push('…');
    truncated
}

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let fg = hex_to_color(&app.theme.fg);
    let muted = hex_to_color(&app.theme.muted);
    let accent = hex_to_color(&app.theme.accent);
    let plain = Style::default();
    let sep = Span::styled(" │ ", plain.fg(muted));

    let mut spans = Vec::new();

    if let Some(label) = mode_label(app.mode) {
        spans.push(Span::styled(
            format!("{label} "),
            plain.fg(accent).add_modifier(Modifier::BOLD),
        ));
        spans.push(sep.clone());
    }

    let notebook_name = app
        .selected_notebook()
        .map(|nb| nb.name.as_str())
        .unwrap_or("-");
    spans.push(Span::styled(
        format!("{} {notebook_name}", icons::NOTEBOOK),
        plain.fg(fg),
    ));
    spans.push(sep.clone());

    // Contextual metadata: character count of the note actually being read
    // (Notes/Preview, something selected), otherwise how many notes are in
    // view (e.g. while still browsing NOTEBOOKS).
    let meta = match app.selected_note() {
        Some(note) if matches!(app.focus, Focus::Notes | Focus::Preview) => {
            format!("{} {} chars", icons::NOTE, note.body.chars().count())
        }
        _ => format!("{} {} notes", icons::NOTE, app.notes.len()),
    };
    spans.push(Span::styled(meta, plain.fg(fg)));

    // Note version history — how many commits have touched this specific
    // note, only while actually reading one in PREVIEW (not while just
    // browsing NOTES, where it'd compete with the char/note count above).
    if app.focus == Focus::Preview {
        if let Some(count) = app.note_revision_count() {
            spans.push(sep.clone());
            spans.push(Span::styled(
                format!("{} {count} changes", icons::HISTORY),
                plain.fg(muted),
            ));
        }
    }

    if app.git_status.is_repo {
        spans.push(sep.clone());
        let gs = &app.git_status;
        let branch = gs.branch.as_deref().unwrap_or("?");
        let mut extras = String::new();
        if gs.dirty_count > 0 {
            extras.push_str(&format!(" +{}", gs.dirty_count));
        }
        if gs.ahead > 0 {
            extras.push_str(&format!(" {}{}", icons::UPLOAD, gs.ahead));
        }
        if gs.behind > 0 {
            extras.push_str(&format!(" {}{}", icons::DOWNLOAD, gs.behind));
        }
        let color = if gs.dirty_count > 0 {
            hex_to_color(&app.theme.warning)
        } else if gs.ahead > 0 || gs.behind > 0 {
            accent
        } else {
            hex_to_color(&app.theme.success)
        };
        spans.push(Span::styled(
            format!("{} {branch}{extras}", icons::GIT),
            plain.fg(color),
        ));
    }

    spans.push(sep.clone());
    let editor_color = if app.config.general.use_favorite_editor {
        accent
    } else {
        muted
    };
    spans.push(Span::styled(
        format!("{} {}", icons::PENCIL, app.editor_status_label()),
        plain.fg(editor_color),
    ));

    if app.leader_pending {
        spans.push(sep.clone());
        spans.push(Span::styled(
            format!("{} leader…", icons::KEYBOARD),
            plain.fg(accent).add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(status) = &app.status_message {
        // Truncated to whatever room is actually left, so a long message
        // can't push the right-aligned help/version text out of view or
        // overlap it — reserves a rough estimate for that right side since
        // it isn't computed from the same span list.
        const RESERVED_RIGHT: usize = 24;
        let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        let budget = (area.width as usize)
            .saturating_sub(used)
            .saturating_sub(RESERVED_RIGHT);
        if budget > 1 {
            spans.push(sep);
            spans.push(Span::styled(truncate_to(status, budget), plain.fg(fg)));
        }
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);

    // Right-aligned over the same area — no background is painted anywhere
    // on this bar, so this just draws on top of empty space rather than
    // needing a separately carved-out sub-rect.
    let right = Paragraph::new(Line::from(Span::styled(
        format!(
            "  {} ? help   v{}  ",
            icons::KEYBOARD,
            env!("CARGO_PKG_VERSION")
        ),
        plain.fg(muted),
    )))
    .alignment(Alignment::Right);
    frame.render_widget(right, area);
}
