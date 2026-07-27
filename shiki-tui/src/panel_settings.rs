use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, List, ListItem, ListState};
use ratatui::Frame;

use crate::app::App;
use crate::icons;
use crate::render::{hex_to_color, panel_block};

/// Builds the Settings screen's read-only summary as one `Line` per row —
/// section headers styled distinctly from their key/value rows underneath.
/// Rebuilt on every keypress/render rather than cached: it's a handful of
/// short strings read straight out of `app.config`, nowhere near the
/// per-frame cost that made `note_preview_cache`/`folder_preview_cache`
/// worth adding for genuinely large content.
///
/// Deliberately doesn't repeat the keybindings tables (`[keybindings.*]`) —
/// `?` (which-key) already covers every one of those live, generated from
/// this exact config, so it can't drift; duplicating that here would just
/// be a second copy to keep in sync. This summary covers what which-key
/// doesn't: general/theme/git/per-notebook overrides/snippets.
// The section rows below mix unconditional pushes with conditional loops
// (NOTEBOOKS/SNIPPETS have a variable number of rows) — a `vec![]` literal
// can't express that, so the whole function stays as sequential `push`es
// for one consistent shape throughout, rather than a `vec![]` prefix (fixed
// rows) glued to `.extend(...)` (variable ones).
#[allow(clippy::vec_init_then_push)]
pub fn build(app: &App) -> Vec<Line<'static>> {
    let accent = hex_to_color(&app.theme.accent);
    let fg = hex_to_color(&app.theme.fg);
    let muted = hex_to_color(&app.theme.muted);

    let header = move |text: String| {
        Line::from(Span::styled(
            text,
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ))
    };
    let row = move |label: &str, value: String| {
        Line::from(vec![
            Span::styled(format!("  {label:<20}"), Style::default().fg(muted)),
            Span::styled(value, Style::default().fg(fg)),
        ])
    };

    let cfg = &app.config;
    let mut lines = Vec::new();

    lines.push(header("GENERAL".into()));
    lines.push(row(
        "default_notebook",
        cfg.general.default_notebook.clone(),
    ));
    lines.push(row("editor", cfg.general.editor.clone()));
    lines.push(row("daily_template", cfg.general.daily_template.clone()));
    lines.push(row(
        "use_favorite_editor",
        cfg.general.use_favorite_editor.to_string(),
    ));
    lines.push(Line::default());

    lines.push(header("THEME".into()));
    lines.push(row("name", cfg.theme.name.clone()));
    let set = cfg.theme.overrides.set_count();
    lines.push(row(
        "overrides",
        if set == 0 {
            "none".to_string()
        } else {
            format!("{set} of 19 slots set")
        },
    ));
    lines.push(Line::default());

    lines.push(header(
        "GIT (defaults — per-notebook overrides below)".into(),
    ));
    lines.push(row("auto_commit", cfg.git.auto_commit.to_string()));
    lines.push(row("auto_push", cfg.git.auto_push.to_string()));
    lines.push(row("commit_prefix", cfg.git.commit_prefix.clone()));
    lines.push(row("remote", cfg.git.remote.clone()));
    lines.push(row("branch", cfg.git.branch.clone()));
    lines.push(row("sign_commits", cfg.git.sign_commits.to_string()));
    lines.push(row("auto_sync", cfg.git.auto_sync.to_string()));
    lines.push(row("auto_sync_every", cfg.git.auto_sync_every.to_string()));
    lines.push(row(
        "remote_template",
        if cfg.git.remote_template.is_empty() {
            "(none)".to_string()
        } else {
            cfg.git.remote_template.clone()
        },
    ));
    lines.push(Line::default());

    lines.push(header("NOTEBOOKS (per-notebook git overrides)".into()));
    if cfg.notebooks.is_empty() {
        lines.push(row("", "none configured".to_string()));
    } else {
        let mut names: Vec<&String> = cfg.notebooks.keys().collect();
        names.sort();
        for name in names {
            let over = &cfg.notebooks[name];
            let mut parts = Vec::new();
            if let Some(v) = over.auto_push {
                parts.push(format!("auto_push={v}"));
            }
            if let Some(v) = over.auto_sync {
                parts.push(format!("auto_sync={v}"));
            }
            if let Some(v) = over.auto_sync_every {
                parts.push(format!("auto_sync_every={v}"));
            }
            lines.push(row(name, parts.join(", ")));
        }
    }
    lines.push(Line::default());

    lines.push(header("SNIPPETS (/-menu custom commands)".into()));
    if cfg.snippets.is_empty() {
        lines.push(row(
            "",
            "none configured — see [snippets.<trigger>] in config.toml".to_string(),
        ));
    } else {
        let mut triggers: Vec<&String> = cfg.snippets.keys().collect();
        triggers.sort();
        for trigger in triggers {
            let snippet = &cfg.snippets[trigger];
            let label = snippet.label.clone().unwrap_or_else(|| trigger.clone());
            lines.push(row(trigger, label));
        }
    }

    lines
}

/// Near-full-screen popup, same sizing convention as which-key — a summary
/// this size deserves more room than a small centered box. Read-only
/// (there's no per-row action), so `j`/`k`/`PageUp`/`PageDown`/`Home`/`End`
/// only ever move the scroll position; `i`/`E` jump straight to editing
/// `config.toml` itself (see `App::handle_settings_key`).
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let margin_x = area.width / 10;
    let margin_y = area.height / 10;
    let popup_area = Rect {
        x: area.x + margin_x,
        y: area.y + margin_y,
        width: area.width.saturating_sub(margin_x * 2),
        height: area.height.saturating_sub(margin_y * 2),
    };
    frame.render_widget(Clear, popup_area);

    let lines = build(app);
    let items: Vec<ListItem> = lines.into_iter().map(ListItem::new).collect();

    let title = format!(
        " {}  Settings — i edit inline · E edit external · esc/q close ",
        icons::GEAR
    );
    let list = List::new(items)
        .block(panel_block(Line::from(title), true, &app.theme))
        .highlight_style(
            Style::default()
                .bg(hex_to_color(&app.theme.selection))
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    state.select(Some(app.settings_selected));
    frame.render_stateful_widget(list, popup_area, &mut state);
}
