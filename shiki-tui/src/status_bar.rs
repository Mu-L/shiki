use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, Mode};
use crate::icons;
use crate::render::hex_to_color;

fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Normal => "NORMAL",
        Mode::Insert => "INSERT",
        Mode::Edit => "EDIT",
        Mode::Visual => "VISUAL",
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let bg = hex_to_color(&app.theme.statusbar);
    let fg = hex_to_color(&app.theme.fg);
    let muted = hex_to_color(&app.theme.muted);
    let base = Style::default().bg(bg);

    let notebook_name = app
        .selected_notebook()
        .map(|nb| nb.name.as_str())
        .unwrap_or("-");
    let (sync_icon, sync_color) = if app.git_dirty {
        (icons::WARNING, hex_to_color(&app.theme.warning))
    } else {
        (icons::CHECK, hex_to_color(&app.theme.success))
    };

    let mut spans = vec![
        Span::styled(
            format!(" {} ", mode_label(app.mode)),
            base.fg(hex_to_color(&app.theme.accent))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" │ ", base.fg(muted)),
        Span::styled(format!("{}  {notebook_name}", icons::NOTEBOOK), base.fg(fg)),
        Span::styled(" │ ", base.fg(muted)),
        Span::styled(sync_icon.to_string(), base.fg(sync_color)),
    ];

    if app.leader_pending {
        spans.push(Span::styled("  ", base));
        spans.push(Span::styled(
            format!("{} leader…", icons::KEYBOARD),
            base.fg(hex_to_color(&app.theme.accent))
                .add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(status) = &app.status_message {
        spans.push(Span::styled("  ", base));
        spans.push(Span::styled(status.clone(), base.fg(fg)));
    }

    spans.push(Span::styled(" │ ", base.fg(muted)));
    spans.push(Span::styled(app.theme.name.clone(), base.fg(muted)));
    spans.push(Span::styled(" │ ", base.fg(muted)));
    spans.push(Span::styled(
        format!("{} ? for help", icons::KEYBOARD),
        base.fg(muted),
    ));

    frame.render_widget(Paragraph::new(Line::from(spans)).style(base), area);

    // Right-aligned over the same area: the left Paragraph above already
    // painted the whole row's background, so this just draws on top of it
    // rather than needing a separately carved-out sub-rect.
    let version = Paragraph::new(Line::from(Span::styled(
        format!(" v{} ", env!("CARGO_PKG_VERSION")),
        base.fg(muted),
    )))
    .alignment(Alignment::Right);
    frame.render_widget(version, area);
}
