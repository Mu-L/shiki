use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::app::Focus;

/// Screen areas: 3 panels on top + status bar at the bottom.
pub struct Areas {
    pub notebooks: Rect,
    pub notes: Rect,
    pub preview: Rect,
    pub status_bar: Rect,
}

/// Width a panel shrinks to when it's not part of the current reading path —
/// just its border line, so it stays visible but out of the way (Yazi-style
/// Miller columns) instead of competing for space with the panel you're
/// actually using. One column is enough: the user already knows there's a
/// collapsed panel there from the border alone, so there's no need to spend
/// extra columns showing a sliver of content nobody can read at that width.
const COLLAPSED: u16 = 1;

pub fn split(area: Rect, focus: Focus) -> Areas {
    // No outer margin and no gap between constraints: panels go edge-to-edge
    // with the terminal and with each other, so the only "padding" visible
    // anywhere is each panel's own border.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .spacing(0)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let (notebooks_c, notes_c, preview_c) = match focus {
        Focus::Notebooks => (
            Constraint::Fill(1),
            Constraint::Fill(2),
            Constraint::Fill(2),
        ),
        Focus::Notes => (
            Constraint::Length(COLLAPSED),
            Constraint::Fill(1),
            Constraint::Fill(1),
        ),
        Focus::Preview => (
            Constraint::Length(COLLAPSED),
            Constraint::Length(COLLAPSED),
            Constraint::Fill(1),
        ),
    };

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .margin(0)
        .spacing(0)
        .constraints([notebooks_c, notes_c, preview_c])
        .split(rows[0]);

    Areas {
        notebooks: cols[0],
        notes: cols[1],
        preview: cols[2],
        status_bar: rows[1],
    }
}
