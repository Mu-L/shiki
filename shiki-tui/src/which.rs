use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::icons;
use crate::keybindings::{action_icon, action_label, describe_key};
use crate::render::{hex_to_color, panel_block};

/// Centered popup listing every keybinding, grouped by scope (Yazi-style
/// which-key, but segmented: GLOBAL entries need the leader key first,
/// NOTEBOOKS/NOTES/PREVIEW only apply while that panel has focus).
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let keymaps = app.keymaps();
    let entries = keymaps.entries();
    // +1 header line per scope, +1 for the leader hint line, +2 for the block's borders.
    let scopes = entries
        .iter()
        .map(|(scope, ..)| *scope)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let height = (entries.len() as u16 + scopes as u16 + 1 + 2)
        .min(area.height.saturating_sub(2))
        .max(4);
    let width = 46u16.min(area.width.saturating_sub(2));

    let [popup_area] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(area);
    let [popup_area] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(popup_area);

    let accent = hex_to_color(&app.theme.accent);
    let fg = hex_to_color(&app.theme.fg);
    let muted = hex_to_color(&app.theme.muted);

    let mut lines: Vec<Line> = vec![Line::from(vec![
        Span::styled(
            format!("{:>8} ", describe_key(app.keymaps().leader_key())),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled("leader (hold for GLOBAL)", Style::default().fg(muted)),
    ])];

    let mut last_scope: Option<&'static str> = None;
    for (scope, key, action) in entries {
        if last_scope != Some(scope) {
            lines.push(Line::from(Span::styled(
                format!("── {scope} ──"),
                Style::default().fg(muted).add_modifier(Modifier::ITALIC),
            )));
            last_scope = Some(scope);
        }
        lines.push(Line::from(vec![
            Span::styled(
                format!("{key:>8} "),
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{} ", action_icon(action)),
                Style::default().fg(accent),
            ),
            Span::styled(action_label(action), Style::default().fg(fg)),
        ]));
    }

    let title = format!(" {}  Which Key ", icons::KEYBOARD);
    frame.render_widget(Clear, popup_area);
    frame.render_widget(
        Paragraph::new(lines).block(panel_block(Line::from(title), true, &app.theme)),
        popup_area,
    );
}
