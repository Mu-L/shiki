use crate::app::{
    centered_rect, drawer_area, global_search_layout, global_search_popup_area, App, Mode,
    UpdateState,
};
use crate::icons;
use crate::render::{hex_to_color, panel_block};
use crate::{
    layout, panel_drawer, panel_notebooks, panel_notes, panel_preview, panel_tags, status_bar,
    which,
};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::{Clear, List, ListItem, ListState};
use ratatui::Frame;
use shiki_core::TagIndex;

pub fn draw(frame: &mut Frame, app: &App) {
    let background = ratatui::widgets::Block::default()
        .style(ratatui::style::Style::default().bg(hex_to_color(&app.theme.bg)));
    frame.render_widget(background, frame.area());

    let areas = layout::split(frame.area(), app.focus);
    panel_notebooks::render(frame, areas.notebooks, app);
    panel_notes::render(frame, areas.notes, app);
    if app.mode == Mode::Edit {
        if let Some(editor) = &app.editor {
            frame.render_widget(&editor.textarea, areas.preview);
        }
    } else {
        panel_preview::render(frame, areas.preview, app);
    }
    status_bar::render(frame, areas.status_bar, app);

    if let Some(kind) = app.pending_input {
        let popup_area = centered_rect(frame.area(), (frame.area().width / 2).max(30), 3);
        frame.render_widget(Clear, popup_area);
        let title = app
            .pending_input_title
            .as_deref()
            .unwrap_or_else(|| kind.title());
        app.input
            .render(frame, popup_area, title, hex_to_color(&app.theme.accent));
    }

    if app.show_tags {
        let rows = match &app.tags_viewing {
            None => TagIndex::build(&app.notes).len(),
            Some(tag) => app
                .notes
                .iter()
                .filter(|n| n.frontmatter.tags.iter().any(|t| t == tag))
                .count()
                .max(1),
        };
        let popup_area = centered_rect(frame.area(), 40, (rows as u16 + 2).max(3));
        frame.render_widget(Clear, popup_area);
        panel_tags::render(frame, popup_area, app);
    }

    if app.show_drawer {
        let drawer_rect = drawer_area(frame.area());
        frame.render_widget(Clear, drawer_rect);
        panel_drawer::render(frame, drawer_rect, app);
    }

    if app.show_theme_picker {
        render_theme_picker(frame, frame.area(), app);
    }

    if app.show_template_picker {
        render_template_picker(frame, frame.area(), app);
    }

    if app.show_global_search {
        render_global_search(frame, frame.area(), app);
    }

    if app.show_logs {
        render_logs(frame, frame.area(), app);
    }

    if app.show_tree {
        render_tree(frame, frame.area(), app);
    }

    if app.show_links {
        render_links(frame, frame.area(), app);
    }

    if app.show_history {
        render_history(frame, frame.area(), app);
    }

    if app.show_update {
        render_update(frame, frame.area(), app);
    }

    if app.show_which_key {
        which::render(frame, frame.area(), app);
    }

    if let Some(dialog) = &app.confirm {
        let popup_area = centered_rect(frame.area(), (dialog.message.len() as u16 + 12).max(30), 3);
        frame.render_widget(Clear, popup_area);
        dialog.render(frame, popup_area, hex_to_color(&app.theme.warning));
    }
}

fn render_theme_picker(frame: &mut Frame, frame_area: Rect, app: &App) {
    let height = (app.available_themes.len() as u16 + 2).min(frame_area.height.saturating_sub(2));
    let popup_area = centered_rect(frame_area, 40, height);
    frame.render_widget(Clear, popup_area);

    let items: Vec<ListItem> = app
        .available_themes
        .iter()
        .map(|t| ListItem::new(t.name.clone()))
        .collect();
    let highlight_symbol = format!("{} ", icons::ARROW);
    let title = format!(" {}  Pick a theme ", icons::EYE);
    let list = List::new(items)
        .block(panel_block(Line::from(title), true, &app.theme))
        .highlight_style(
            Style::default()
                .bg(hex_to_color(&app.theme.selection))
                .fg(hex_to_color(&app.theme.accent))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(&highlight_symbol);

    let mut state = ListState::default();
    state.select(Some(app.theme_picker_index));
    frame.render_stateful_widget(list, popup_area, &mut state);
}

fn render_template_picker(frame: &mut Frame, frame_area: Rect, app: &App) {
    let height =
        (app.template_picker_options.len() as u16 + 2).min(frame_area.height.saturating_sub(2));
    let popup_area = centered_rect(frame_area, 40, height);
    frame.render_widget(Clear, popup_area);

    let items: Vec<ListItem> = app
        .template_picker_options
        .iter()
        .map(|opt| match opt {
            Some(name) => ListItem::new(name.clone()),
            None => ListItem::new("(blank, no template)"),
        })
        .collect();
    let highlight_symbol = format!("{} ", icons::ARROW);
    let title = format!(" {}  Pick a template ", icons::NOTE);
    let list = List::new(items)
        .block(panel_block(Line::from(title), true, &app.theme))
        .highlight_style(
            Style::default()
                .bg(hex_to_color(&app.theme.selection))
                .fg(hex_to_color(&app.theme.accent))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(&highlight_symbol);

    let mut state = ListState::default();
    state.select(Some(app.template_picker_index));
    frame.render_stateful_widget(list, popup_area, &mut state);
}

fn render_global_search(frame: &mut Frame, frame_area: Rect, app: &App) {
    let popup_area = global_search_popup_area(frame_area);
    frame.render_widget(Clear, popup_area);
    let (input_area, list_area) = global_search_layout(popup_area);

    app.global_search_input.render(
        frame,
        input_area,
        &format!(" {}  Search all notes ", icons::SEARCH),
        hex_to_color(&app.theme.accent),
    );

    let items: Vec<ListItem> = app
        .global_search_results
        .iter()
        .filter_map(|hit| {
            let (nb, note) = app.global_search_pool.get(hit.index)?;
            Some(ListItem::new(format!(
                "{}  {} \u{203A} {}",
                icons::NOTE,
                nb.name,
                note.frontmatter.title
            )))
        })
        .collect();
    let highlight_symbol = format!("{} ", icons::ARROW);
    let count = app.global_search_results.len();
    let title = format!(" Results [{count}] ");
    let list = List::new(items)
        .block(panel_block(Line::from(title), true, &app.theme))
        .highlight_style(
            Style::default()
                .bg(hex_to_color(&app.theme.selection))
                .fg(hex_to_color(&app.theme.accent))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(&highlight_symbol);

    let mut state = ListState::default();
    if !app.global_search_results.is_empty() {
        state.select(Some(app.global_search_selected));
    }
    frame.render_stateful_widget(list, list_area, &mut state);
}

fn render_update(frame: &mut Frame, frame_area: Rect, app: &App) {
    let popup_area = centered_rect(frame_area, (frame_area.width * 2 / 3).max(50), 7);
    frame.render_widget(Clear, popup_area);

    let current = env!("CARGO_PKG_VERSION");
    let (title, body) = match &app.update_state {
        Some(UpdateState::Checking) => (
            " Checking for updates ".to_string(),
            "Checking GitHub Releases\u{2026}".to_string(),
        ),
        Some(UpdateState::Available(version)) => (
            format!(" {}  Update available ", icons::DOWNLOAD),
            format!("v{current} \u{2192} v{version}\n\n[enter] Download & install    [esc] Cancel"),
        ),
        Some(UpdateState::UpToDate) => (
            " Up to date ".to_string(),
            format!("You're on the latest version (v{current}).\n\n[esc] Close"),
        ),
        Some(UpdateState::Downloading) => (
            " Installing ".to_string(),
            "Downloading, verifying, and installing\u{2026}".to_string(),
        ),
        Some(UpdateState::Installed(version)) => (
            " Installed ".to_string(),
            format!("Installed v{version} \u{2014} restarting shiki\u{2026}"),
        ),
        Some(UpdateState::Error(message)) => (
            " Update failed ".to_string(),
            format!("{message}\n\n[esc] Close"),
        ),
        None => (" Update ".to_string(), String::new()),
    };

    let paragraph = ratatui::widgets::Paragraph::new(body)
        .wrap(ratatui::widgets::Wrap { trim: false })
        .block(panel_block(Line::from(title), true, &app.theme));
    frame.render_widget(paragraph, popup_area);
}

fn render_logs(frame: &mut Frame, frame_area: Rect, app: &App) {
    let height = (frame_area.height * 2 / 3).max(6);
    let popup_area = centered_rect(frame_area, (frame_area.width * 3 / 4).max(40), height);
    frame.render_widget(Clear, popup_area);

    let items: Vec<ListItem> = app
        .log_history
        .iter()
        .map(|entry| {
            ListItem::new(format!(
                "{}  {}",
                entry.at.format("%H:%M:%S"),
                entry.message
            ))
        })
        .collect();
    let highlight_symbol = format!("{} ", icons::ARROW);
    let title = format!(
        " {}  Logs [{}]  \u{2014}  y/c copy all \u{B7} esc/q close ",
        icons::LIST,
        app.log_history.len()
    );
    let list = List::new(items)
        .block(panel_block(Line::from(title), true, &app.theme))
        .highlight_style(
            Style::default()
                .bg(hex_to_color(&app.theme.selection))
                .fg(hex_to_color(&app.theme.accent))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(&highlight_symbol);

    let mut state = ListState::default();
    if !app.log_history.is_empty() {
        state.select(Some(app.logs_selected));
    }
    frame.render_stateful_widget(list, popup_area, &mut state);
}

fn render_tree(frame: &mut Frame, frame_area: Rect, app: &App) {
    let height = (frame_area.height * 3 / 4).max(8);
    let popup_area = centered_rect(frame_area, (frame_area.width * 3 / 4).max(40), height);
    frame.render_widget(Clear, popup_area);

    let muted = hex_to_color(&app.theme.muted);
    let fg = hex_to_color(&app.theme.fg);
    let items: Vec<ListItem> = app
        .tree_rows
        .iter()
        .map(|row| match row {
            crate::tree::TreeRow::Folder { depth, name } => {
                ListItem::new(Line::from(Span::styled(
                    format!("{}{} {name}/", "  ".repeat(*depth), icons::NOTEBOOK),
                    Style::default().fg(muted).add_modifier(Modifier::BOLD),
                )))
            }
            crate::tree::TreeRow::Note { depth, note } => ListItem::new(Line::from(Span::styled(
                format!(
                    "{}{} {}",
                    "  ".repeat(*depth),
                    icons::NOTE,
                    note.frontmatter.title
                ),
                Style::default().fg(fg),
            ))),
        })
        .collect();
    let highlight_symbol = format!("{} ", icons::ARROW);
    let title = format!(
        " {}  Tree [{} notes]  \u{2014}  enter open \u{B7} esc/q close ",
        icons::TREE,
        app.tree_note_count()
    );
    let list = List::new(items)
        .block(panel_block(Line::from(title), true, &app.theme))
        .highlight_style(
            Style::default()
                .bg(hex_to_color(&app.theme.selection))
                .fg(hex_to_color(&app.theme.accent))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(&highlight_symbol);

    let mut state = ListState::default();
    state.select(app.tree_selected_row());
    frame.render_stateful_widget(list, popup_area, &mut state);
}

fn render_links(frame: &mut Frame, frame_area: Rect, app: &App) {
    let height = ((app.link_rows.len() as u16) + 2).max(4);
    let popup_area = centered_rect(frame_area, (frame_area.width * 3 / 4).max(40), height);
    frame.render_widget(Clear, popup_area);

    let muted = hex_to_color(&app.theme.muted);
    let fg = hex_to_color(&app.theme.fg);
    let error = hex_to_color(&app.theme.error);
    let items: Vec<ListItem> = app
        .link_rows
        .iter()
        .map(|row| match row {
            crate::links_panel::LinkRow::Header(label) => ListItem::new(Line::from(Span::styled(
                label.to_string(),
                Style::default().fg(muted).add_modifier(Modifier::BOLD),
            ))),
            crate::links_panel::LinkRow::Outgoing {
                text,
                resolved: Some(_),
            } => ListItem::new(Line::from(Span::styled(
                format!("  {} {text}", icons::LINK),
                Style::default().fg(fg),
            ))),
            crate::links_panel::LinkRow::Outgoing {
                text,
                resolved: None,
            } => ListItem::new(Line::from(Span::styled(
                format!("  {} {text}  (no matching note)", icons::WARNING),
                Style::default().fg(error),
            ))),
            crate::links_panel::LinkRow::Backlink { note } => {
                ListItem::new(Line::from(Span::styled(
                    format!("  {} {}", icons::NOTE, note.frontmatter.title),
                    Style::default().fg(fg),
                )))
            }
        })
        .collect();
    let highlight_symbol = format!("{} ", icons::ARROW);
    let title = format!(
        " {}  Links  \u{2014}  enter jump \u{B7} esc/q close ",
        icons::LINK
    );
    let list = List::new(items)
        .block(panel_block(Line::from(title), true, &app.theme))
        .highlight_style(
            Style::default()
                .bg(hex_to_color(&app.theme.selection))
                .fg(hex_to_color(&app.theme.accent))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(&highlight_symbol);

    let mut state = ListState::default();
    state.select(crate::links_panel::selected_row(
        &app.link_rows,
        app.link_selected,
    ));
    frame.render_stateful_widget(list, popup_area, &mut state);
}

fn render_history(frame: &mut Frame, frame_area: Rect, app: &App) {
    let height = (frame_area.height * 3 / 4).max(8);
    let popup_area = centered_rect(frame_area, (frame_area.width * 3 / 4).max(50), height);
    frame.render_widget(Clear, popup_area);

    let fg = hex_to_color(&app.theme.fg);
    let muted = hex_to_color(&app.theme.muted);

    if let Some((commit_id, content)) = &app.history_viewing {
        let short = commit_id.chars().take(7).collect::<String>();
        let title = format!(
            " {}  Revision {short}  \u{2014}  r revert \u{B7} esc back ",
            icons::HISTORY
        );
        let paragraph = ratatui::widgets::Paragraph::new(content.as_str())
            .block(panel_block(Line::from(title), true, &app.theme))
            .wrap(ratatui::widgets::Wrap { trim: false });
        frame.render_widget(paragraph, popup_area);
        return;
    }

    let items: Vec<ListItem> = app
        .history_entries
        .iter()
        .map(|entry| {
            let short = entry.commit_id.chars().take(7).collect::<String>();
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{} ", entry.date.format("%Y-%m-%d %H:%M")),
                    Style::default().fg(fg),
                ),
                Span::styled(format!("{short}  "), Style::default().fg(muted)),
                Span::styled(entry.message.clone(), Style::default().fg(fg)),
            ]))
        })
        .collect();
    let highlight_symbol = format!("{} ", icons::ARROW);
    let title = format!(
        " {}  History [{} revisions]  \u{2014}  enter view \u{B7} r revert \u{B7} esc/q close ",
        icons::HISTORY,
        app.history_entries.len()
    );
    let list = List::new(items)
        .block(panel_block(Line::from(title), true, &app.theme))
        .highlight_style(
            Style::default()
                .bg(hex_to_color(&app.theme.selection))
                .fg(hex_to_color(&app.theme.accent))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(&highlight_symbol);

    let mut state = ListState::default();
    if !app.history_entries.is_empty() {
        state.select(Some(app.history_selected));
    }
    frame.render_stateful_widget(list, popup_area, &mut state);
}
