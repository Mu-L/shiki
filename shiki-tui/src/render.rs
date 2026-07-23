use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Scrollbar, ScrollbarOrientation, ScrollbarState,
};
use ratatui::Frame;
use shiki_config::Theme;

/// Converts a theme color slot to `ratatui::Color`. Accepts `#rrggbb` hex
/// (every built-in palette), the terminal's native ANSI names, or `"reset"`
/// to inherit whatever the terminal's own default color is — that's what
/// the "default" theme uses, so it looks right in any terminal color scheme
/// instead of imposing a fixed palette on top of it.
pub fn hex_to_color(value: &str) -> Color {
    match value.to_ascii_lowercase().as_str() {
        "reset" | "" => return Color::Reset,
        "black" => return Color::Black,
        "red" => return Color::Red,
        "green" => return Color::Green,
        "yellow" => return Color::Yellow,
        "blue" => return Color::Blue,
        "magenta" => return Color::Magenta,
        "cyan" => return Color::Cyan,
        "white" => return Color::White,
        "gray" | "grey" => return Color::Gray,
        "darkgray" | "darkgrey" => return Color::DarkGray,
        _ => {}
    }
    let hex = value.trim_start_matches('#');
    if hex.len() != 6 {
        return Color::Reset;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    Color::Rgb(r, g, b)
}

/// Themed panel `Block`: fills bg/fg from the theme, uses a thicker accent
/// border when focused and a subtle rounded one otherwise. Shared by every
/// panel and popup so the whole UI reads as one consistent surface instead of
/// bare unstyled borders on the terminal's default background.
pub fn panel_block<'a>(title: impl Into<Line<'a>>, focused: bool, theme: &Theme) -> Block<'a> {
    let border_color = if focused {
        hex_to_color(&theme.accent)
    } else {
        hex_to_color(&theme.border)
    };
    Block::default()
        .title(title)
        .title_style(
            Style::default()
                .fg(hex_to_color(&theme.panel_title))
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_type(if focused {
            BorderType::Thick
        } else {
            BorderType::Rounded
        })
        .border_style(Style::default().fg(border_color))
        .style(
            Style::default()
                .bg(hex_to_color(&theme.bg))
                .fg(hex_to_color(&theme.fg)),
        )
}

/// Renders a vertical scrollbar along the right edge of `area` for a list of
/// `total` items currently positioned at `selected`. No-op when everything
/// fits, so short lists don't grow a useless track.
pub fn render_scrollbar(
    frame: &mut Frame,
    area: Rect,
    total: usize,
    selected: usize,
    theme: &Theme,
) {
    if total <= area.height.saturating_sub(2) as usize {
        return;
    }
    let mut state = ScrollbarState::new(total).position(selected);
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .track_symbol(None)
        .style(Style::default().fg(hex_to_color(&theme.scrollbar)));
    frame.render_stateful_widget(
        scrollbar,
        area.inner(ratatui::layout::Margin::new(0, 1)),
        &mut state,
    );
}

/// Splits `text` on `[[wikilink]]` occurrences, styling the links distinctly
/// from the surrounding prose so cross-references stand out in the preview.
fn wikilink_spans(text: &str, base: Style, link: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        if start > 0 {
            spans.push(Span::styled(rest[..start].to_string(), base));
        }
        let after = &rest[start + 2..];
        match after.find("]]") {
            Some(end) => {
                spans.push(Span::styled(format!("[[{}]]", &after[..end]), link));
                rest = &after[end + 2..];
            }
            None => {
                spans.push(Span::styled(rest[start..].to_string(), base));
                rest = "";
                break;
            }
        }
    }
    if !rest.is_empty() || spans.is_empty() {
        spans.push(Span::styled(rest.to_string(), base));
    }
    spans
}

/// Minimal markdown-to-styled-lines render: bold headings, checkbox/list
/// bullets, dimmed code fences and blockquotes, styled `[[wikilinks]]`. Good
/// enough for the preview panel; not a replacement for a full
/// comrak/syntect-based renderer.
/// Rebuilds `Line<'a>`s that borrow their text from an existing
/// `&'a [Line<'static>]` instead of cloning it — used to hand a cached,
/// already-formatted preview (`App::note_preview_lines`/
/// `folder_preview_lines`) to `Paragraph::new`, which needs an owned
/// `Vec<Line<'a>>`. A plain `.to_vec()` would deep-clone every `String`
/// backing every span — cheap for a handful of lines, but a real,
/// measured cost on a huge note or a folder with tens of thousands of
/// entries, re-paid on every single draw tick even though the *content*
/// hasn't changed. Only the small `Style`/`Vec` scaffolding gets rebuilt
/// here; every span's actual text is a `Cow::Borrowed` pointing at the
/// cache's own bytes, so nothing text-sized gets copied.
pub fn borrow_lines<'a>(lines: &'a [Line<'static>]) -> Vec<Line<'a>> {
    lines
        .iter()
        .map(|line| Line {
            style: line.style,
            alignment: line.alignment,
            spans: line
                .spans
                .iter()
                .map(|span| Span {
                    style: span.style,
                    content: std::borrow::Cow::Borrowed(span.content.as_ref()),
                })
                .collect(),
        })
        .collect()
}

pub fn markdown_to_lines(
    body: &str,
    fg: Color,
    accent: Color,
    muted: Color,
    link: Color,
) -> Vec<Line<'static>> {
    let heading = Style::default().fg(accent).add_modifier(Modifier::BOLD);
    let text = Style::default().fg(fg);
    let dim = Style::default().fg(muted).add_modifier(Modifier::ITALIC);
    let link_style = Style::default().fg(link).add_modifier(Modifier::UNDERLINED);

    let mut in_code_block = false;
    let mut lines = Vec::new();

    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            lines.push(Line::from(Span::styled(line.to_string(), dim)));
            continue;
        }
        if in_code_block {
            lines.push(Line::from(Span::styled(line.to_string(), dim)));
            continue;
        }

        lines.push(if let Some(rest) = line.strip_prefix("### ") {
            Line::from(Span::styled(rest.to_string(), heading))
        } else if let Some(rest) = line.strip_prefix("## ") {
            Line::from(Span::styled(rest.to_string(), heading))
        } else if let Some(rest) = line.strip_prefix("# ") {
            Line::from(Span::styled(
                rest.to_string(),
                heading.add_modifier(Modifier::UNDERLINED),
            ))
        } else if let Some(rest) = line.strip_prefix("- [x] ").or(line.strip_prefix("- [X] ")) {
            Line::from(vec![
                Span::styled(
                    format!("{} ", crate::icons::CHECK),
                    Style::default().fg(accent),
                ),
                Span::styled(
                    rest.to_string(),
                    Style::default()
                        .fg(muted)
                        .add_modifier(Modifier::CROSSED_OUT),
                ),
            ])
        } else if let Some(rest) = line.strip_prefix("- [ ] ") {
            let mut spans = vec![Span::styled("☐ ", Style::default().fg(muted))];
            spans.extend(wikilink_spans(rest, text, link_style));
            Line::from(spans)
        } else if let Some(rest) = line.strip_prefix("- ") {
            let mut spans = vec![Span::styled("• ", Style::default().fg(accent))];
            spans.extend(wikilink_spans(rest, text, link_style));
            Line::from(spans)
        } else if let Some(rest) = line.strip_prefix("> ") {
            Line::from(Span::styled(format!("▏ {rest}"), dim))
        } else {
            Line::from(wikilink_spans(line, text, link_style))
        });
    }

    lines
}
