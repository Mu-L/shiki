//! Pre-wraps PREVIEW's rendered lines ourselves instead of letting
//! `ratatui::widgets::Paragraph`'s `Wrap` do it internally. `Paragraph`
//! never exposes the row boundaries it computes, and `preview_scroll`
//! scrolls in that same invisible space — so mapping a mouse click back to
//! a row, or highlighting a drag-selected range, would otherwise mean
//! reverse-engineering a private, unversioned ratatui internal
//! (`widgets::reflow`). Wrapping here instead makes the row boundaries
//! exact by construction: whatever this produces is exactly what's
//! rendered and exactly what `preview_scroll`/hit-testing operate on.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// A maximal run of same-class (whitespace vs. non-whitespace) characters,
/// possibly spanning multiple source `Span`s (e.g. a word split mid-way by
/// inline markdown styling) — `pieces` keeps each sub-run's own style so
/// reassembly doesn't lose it.
struct Atom {
    pieces: Vec<(String, Style)>,
    width: usize,
    whitespace: bool,
}

/// Greedily word-wraps every `Line` in `lines` to `width` columns, breaking
/// at whitespace boundaries and preserving each span's `Style` across a
/// split. A single word wider than `width` is hard-broken character by
/// character, since there's no whitespace boundary to prefer within it.
/// `width == 0` returns `lines` unchanged — there's nothing sensible to
/// wrap into.
pub fn wrap_lines(lines: &[Line<'static>], width: u16) -> Vec<Line<'static>> {
    if width == 0 {
        return lines.to_vec();
    }
    let width = width as usize;
    let mut out = Vec::with_capacity(lines.len());
    for line in lines {
        let atoms = line_to_atoms(line);
        for row in pack_atoms(atoms, width) {
            out.push(atoms_to_line(row));
        }
    }
    out
}

fn line_to_atoms(line: &Line<'static>) -> Vec<Atom> {
    let mut atoms: Vec<Atom> = Vec::new();
    let mut current: Option<Atom> = None;

    for span in &line.spans {
        let style = span.style;
        let text = span.content.as_ref();
        let mut pos = 0;
        while pos < text.len() {
            let is_ws = text[pos..].chars().next().is_some_and(char::is_whitespace);
            let mut end = pos;
            for (idx, ch) in text[pos..].char_indices() {
                if ch.is_whitespace() != is_ws {
                    break;
                }
                end = pos + idx + ch.len_utf8();
            }
            let run = &text[pos..end];
            let run_width = UnicodeWidthStr::width(run);

            match &mut current {
                Some(atom) if atom.whitespace == is_ws => {
                    atom.pieces.push((run.to_string(), style));
                    atom.width += run_width;
                }
                _ => {
                    if let Some(prev) = current.take() {
                        atoms.push(prev);
                    }
                    current = Some(Atom {
                        pieces: vec![(run.to_string(), style)],
                        width: run_width,
                        whitespace: is_ws,
                    });
                }
            }
            pos = end;
        }
    }
    if let Some(last) = current.take() {
        atoms.push(last);
    }
    atoms
}

/// Splits a non-whitespace atom wider than `width` into multiple atoms of
/// at most `width` columns each, preserving per-character style. A no-op
/// (returns `vec![atom]`) when the atom already fits.
fn split_oversized(atom: Atom, width: usize) -> Vec<Atom> {
    if atom.width <= width {
        return vec![atom];
    }
    let mut result = Vec::new();
    let mut pieces: Vec<(String, Style)> = Vec::new();
    let mut cur_width = 0usize;

    for (text, style) in atom.pieces {
        for ch in text.chars() {
            let ch_width = ch.width().unwrap_or(0);
            if cur_width + ch_width > width && !pieces.is_empty() {
                result.push(Atom {
                    pieces: std::mem::take(&mut pieces),
                    width: cur_width,
                    whitespace: false,
                });
                cur_width = 0;
            }
            match pieces.last_mut() {
                Some((last_text, last_style)) if *last_style == style => last_text.push(ch),
                _ => pieces.push((ch.to_string(), style)),
            }
            cur_width += ch_width;
        }
    }
    if !pieces.is_empty() {
        result.push(Atom {
            pieces,
            width: cur_width,
            whitespace: false,
        });
    }
    result
}

/// Greedy line-packing: adds atoms to the current row while they fit,
/// otherwise starts a new row — dropping a whitespace atom that would
/// dangle at a forced wrap point (standard word-wrap behavior), but never
/// dropping a whitespace-only atom that's the row's own first entry (that's
/// the line's real leading indentation, not a wrap artifact).
fn pack_atoms(atoms: Vec<Atom>, width: usize) -> Vec<Vec<Atom>> {
    let mut expanded = Vec::with_capacity(atoms.len());
    for atom in atoms {
        if atom.whitespace {
            expanded.push(atom);
        } else {
            expanded.extend(split_oversized(atom, width));
        }
    }

    let mut rows: Vec<Vec<Atom>> = Vec::new();
    let mut current: Vec<Atom> = Vec::new();
    let mut current_width = 0usize;

    for atom in expanded {
        if current.is_empty() {
            current_width = atom.width;
            current.push(atom);
        } else if current_width + atom.width <= width {
            current_width += atom.width;
            current.push(atom);
        } else {
            if current.last().is_some_and(|a| a.whitespace) {
                current.pop();
            }
            rows.push(std::mem::take(&mut current));
            current_width = 0;
            if !atom.whitespace {
                current_width = atom.width;
                current.push(atom);
            }
        }
    }
    if !current.is_empty() || rows.is_empty() {
        rows.push(current);
    }
    rows
}

fn atoms_to_line(atoms: Vec<Atom>) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for atom in atoms {
        for (text, style) in atom.pieces {
            match spans.last_mut() {
                Some(last) if last.style == style => {
                    let mut merged = last.content.to_string();
                    merged.push_str(&text);
                    last.content = merged.into();
                }
                _ => spans.push(Span::styled(text, style)),
            }
        }
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Modifier};

    fn plain_text(line: &Line<'static>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn unchanged_when_under_width() {
        let lines = vec![Line::from("hello world")];
        let wrapped = wrap_lines(&lines, 20);
        assert_eq!(wrapped.len(), 1);
        assert_eq!(plain_text(&wrapped[0]), "hello world");
    }

    #[test]
    fn splits_at_whitespace_boundary() {
        let lines = vec![Line::from("hello world")];
        let wrapped = wrap_lines(&lines, 5);
        assert_eq!(wrapped.len(), 2);
        assert_eq!(plain_text(&wrapped[0]), "hello");
        assert_eq!(plain_text(&wrapped[1]), "world");
    }

    #[test]
    fn preserves_style_across_split() {
        let bold = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
        let lines = vec![Line::from(vec![
            Span::raw("hello "),
            Span::styled("world", bold),
        ])];
        let wrapped = wrap_lines(&lines, 5);
        assert_eq!(wrapped.len(), 2);
        assert_eq!(plain_text(&wrapped[0]), "hello");
        assert_eq!(plain_text(&wrapped[1]), "world");
        assert_eq!(wrapped[1].spans[0].style, bold);
    }

    #[test]
    fn hard_breaks_an_overlong_word() {
        let lines = vec![Line::from("abcdefgh")];
        let wrapped = wrap_lines(&lines, 3);
        assert_eq!(wrapped.len(), 3);
        assert_eq!(plain_text(&wrapped[0]), "abc");
        assert_eq!(plain_text(&wrapped[1]), "def");
        assert_eq!(plain_text(&wrapped[2]), "gh");
    }

    #[test]
    fn empty_line_stays_empty() {
        let lines = vec![Line::from("")];
        let wrapped = wrap_lines(&lines, 10);
        assert_eq!(wrapped.len(), 1);
        assert_eq!(plain_text(&wrapped[0]), "");
    }

    #[test]
    fn zero_width_is_a_no_op() {
        let lines = vec![Line::from("hello world")];
        let wrapped = wrap_lines(&lines, 0);
        assert_eq!(wrapped.len(), 1);
        assert_eq!(plain_text(&wrapped[0]), "hello world");
    }
}
