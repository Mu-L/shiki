use std::cell::Cell;

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use tui_textarea::TextArea;

/// Thin wrapper over `tui-textarea` for the inline editor (key `e`/`i`).
///
/// `tui-textarea` 0.7 has no soft line-wrap at all — confirmed against its
/// own `Widget for &TextArea` impl (`widget.rs` in the vendored crate
/// source): it always renders exactly one screen row per *source* line and
/// horizontally scrolls to keep the cursor visible instead
/// (`scroll_top_col`/`next_scroll_top`), with no `set_wrap`-style toggle
/// anywhere in its public API, at any published version. That's fine for
/// the read-only PREVIEW panel, which builds its own plain `Paragraph` and
/// turns on `Wrap { trim: false }` itself (`panel_preview.rs`) — but it
/// meant a long line typed directly into the editor just ran off the edge
/// of the panel instead of wrapping, only becoming visible again once you
/// left edit mode and PREVIEW re-rendered the same text wrapped.
///
/// Rather than forking/patching `tui-textarea` (its `Widget` impl is the
/// only place that would need the change, and it's not a project with
/// fast-merging PRs to depend on for that), `render` bypasses its `Widget`
/// impl entirely and draws the buffer itself — `TextArea` still owns every
/// bit of actual editing (insert/delete/undo/cursor movement/selection all
/// keep going through `editor.textarea.input(key)` unchanged in
/// `key_handlers.rs`), it's just not used to *paint* itself anymore.
/// `wrap_line` computes the word-wrap break points once and reuses the
/// exact same ones both to lay out the visible text and to translate the
/// cursor's logical (row, col) into a screen row/col, so the two can never
/// disagree about where a wrapped line actually breaks.
pub struct InlineEditor<'a> {
    pub textarea: TextArea<'a>,
    /// Topmost visible *visual* (post-wrap) line, auto-following the
    /// cursor on every render — replaces `tui-textarea`'s own private
    /// `Viewport` scroll tracking (which only ever handled horizontal
    /// scroll, and is bypassed along with the rest of its rendering). A
    /// `Cell`, not a plain field, because `render` only borrows `&self` —
    /// the same reason `tui-textarea` itself stores its `Viewport` in an
    /// `AtomicU64` rather than a plain field: rendering can't take `&mut`.
    scroll_top: Cell<u16>,
    /// The cursor's screen row on the *last* render, relative to the
    /// editor's own inner area (post-block, post-scroll) — lets `draw.rs`
    /// anchor the `/`-menu popup right under the current line without
    /// duplicating the wrap/scroll math `render` already does.
    cursor_screen_row: Cell<u16>,
}

impl<'a> InlineEditor<'a> {
    pub fn from_contents(contents: &str) -> Self {
        let lines: Vec<String> = if contents.is_empty() {
            vec![String::new()]
        } else {
            contents.lines().map(str::to_string).collect()
        };
        Self {
            textarea: TextArea::new(lines),
            scroll_top: Cell::new(0),
            cursor_screen_row: Cell::new(0),
        }
    }

    pub fn contents(&self) -> String {
        self.textarea.lines().join("\n")
    }

    /// The cursor's row within the editor's inner area as of the last
    /// `render` call — see the field's own doc comment.
    pub fn cursor_screen_row(&self) -> u16 {
        self.cursor_screen_row.get()
    }

    /// Renders the buffer word-wrapped to `area`'s width, instead of
    /// `tui-textarea`'s own unwrapped-with-horizontal-scroll rendering —
    /// see the struct doc comment for why this exists at all.
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let inner = match self.textarea.block() {
            Some(block) => {
                frame.render_widget(block.clone(), area);
                block.inner(area)
            }
            None => area,
        };
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        let width = inner.width as usize;
        let height = inner.height as usize;

        // Bypassing `tui-textarea`'s own `Widget` impl (see the struct doc
        // comment) means its placeholder-when-empty behavior needs its own
        // reimplementation too — same trigger condition as its `widget.rs`
        // (`!placeholder.is_empty() && self.is_empty()`), reusing
        // `wrap_line` so a long placeholder still wraps instead of running
        // off the edge.
        if !self.textarea.placeholder_text().is_empty() && self.textarea.is_empty() {
            self.scroll_top.set(0);
            self.cursor_screen_row.set(0);
            let chars: Vec<char> = self.textarea.placeholder_text().chars().collect();
            let style = self.textarea.placeholder_style().unwrap_or_default();
            let lines: Vec<Line<'static>> = wrap_line(&chars, width)
                .into_iter()
                .map(|(s, e)| {
                    Line::from(Span::styled(chars[s..e].iter().collect::<String>(), style))
                })
                .collect();
            let paragraph = Paragraph::new(lines).style(self.textarea.style());
            frame.render_widget(paragraph, inner);
            return;
        }

        let source_lines = self.textarea.lines();
        let char_rows: Vec<Vec<char>> = source_lines.iter().map(|l| l.chars().collect()).collect();
        let wraps: Vec<Vec<(usize, usize)>> = char_rows
            .iter()
            .map(|chars| wrap_line(chars, width))
            .collect();

        let (cursor_row, cursor_col) = self.textarea.cursor();
        let (cursor_local_row, _) = locate_in_wrap(&wraps[cursor_row], cursor_col);
        let visual_offset_of: usize = wraps[..cursor_row].iter().map(Vec::len).sum();
        let cursor_visual_row = visual_offset_of + cursor_local_row;
        let total_visual_rows: usize = wraps.iter().map(Vec::len).sum();

        let mut scroll_top = next_scroll_top(
            self.scroll_top.get(),
            cursor_visual_row as u16,
            height as u16,
        );
        let max_scroll = total_visual_rows.saturating_sub(height) as u16;
        scroll_top = scroll_top.min(max_scroll);
        self.scroll_top.set(scroll_top);
        let scroll_top = scroll_top as usize;
        self.cursor_screen_row
            .set((cursor_visual_row as u16).saturating_sub(scroll_top as u16));

        let styles = RowStyles {
            base: self.textarea.style(),
            cursor: self.textarea.cursor_style(),
            cursor_line: self.textarea.cursor_line_style(),
            // `tui-textarea` only exposes `selection_style` via a getter
            // that oddly takes `&mut self` (a quirk of its 0.7.0 API) and
            // shiki never calls `set_selection_style`, so this is the same
            // value `TextArea::default()` already uses internally.
            select: Style::default().bg(Color::LightBlue),
        };
        let selection = self.textarea.selection_range();

        let mut rendered: Vec<Line<'static>> = Vec::with_capacity(height);
        let mut visual_row = 0usize;
        'rows: for (row, chars) in char_rows.iter().enumerate() {
            for (local_idx, &seg) in wraps[row].iter().enumerate() {
                if visual_row >= scroll_top + height {
                    break 'rows;
                }
                if visual_row >= scroll_top {
                    let ctx = SegmentCtx {
                        row,
                        cursor_row,
                        cursor_col,
                        is_cursor_segment: row == cursor_row && local_idx == cursor_local_row,
                        selection,
                    };
                    rendered.push(build_segment_line(chars, seg, &ctx, &styles));
                }
                visual_row += 1;
            }
        }

        let paragraph = Paragraph::new(rendered)
            .style(styles.base)
            .alignment(self.textarea.alignment());
        frame.render_widget(paragraph, inner);
    }
}

struct RowStyles {
    base: Style,
    cursor: Style,
    cursor_line: Style,
    select: Style,
}

struct SegmentCtx {
    row: usize,
    cursor_row: usize,
    cursor_col: usize,
    /// Whether *this* wrapped segment (not just `row`) is the one
    /// `locate_in_wrap` picked as the cursor's visual line — a row that
    /// wraps into several segments must highlight the cursor in exactly
    /// one of them, never zero or more than one.
    is_cursor_segment: bool,
    selection: Option<((usize, usize), (usize, usize))>,
}

impl SegmentCtx {
    fn is_selected(&self, col: usize) -> bool {
        let Some(((sr, sc), (er, ec))) = self.selection else {
            return false;
        };
        if self.row < sr || self.row > er {
            false
        } else if sr == er {
            col >= sc && col < ec
        } else if self.row == sr {
            col >= sc
        } else if self.row == er {
            col < ec
        } else {
            true
        }
    }
}

/// Builds one visual (already-wrapped) screen line's spans — plain runs
/// get `cursor_line` style when `ctx.row` is the cursor's row (mirroring
/// `tui-textarea`'s own current-line underline), individual selected
/// characters get `select`, and the cursor's own cell gets `cursor`
/// (a styled space past the last character when the cursor sits at the
/// end of the line, same fallback `tui-textarea`'s `LineHighlighter` uses).
fn build_segment_line(
    chars: &[char],
    (start, end): (usize, usize),
    ctx: &SegmentCtx,
    styles: &RowStyles,
) -> Line<'static> {
    let line_style = if ctx.row == ctx.cursor_row {
        styles.cursor_line
    } else {
        Style::default()
    };
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut plain = String::new();

    for (col, &ch) in chars.iter().enumerate().take(end).skip(start) {
        let is_cursor = ctx.is_cursor_segment && ctx.row == ctx.cursor_row && col == ctx.cursor_col;
        if is_cursor {
            if !plain.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut plain), line_style));
            }
            spans.push(Span::styled(ch.to_string(), styles.cursor));
        } else if ctx.is_selected(col) {
            if !plain.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut plain), line_style));
            }
            spans.push(Span::styled(ch.to_string(), styles.select));
        } else {
            plain.push(ch);
        }
    }
    if !plain.is_empty() {
        spans.push(Span::styled(plain, line_style));
    }
    if ctx.is_cursor_segment && ctx.row == ctx.cursor_row && ctx.cursor_col == end {
        spans.push(Span::styled(" ", styles.cursor));
    }

    Line::from(spans)
}

/// Word-wraps `chars` to `width` columns, returning each visual sub-line's
/// `(start, end)` char-index range (end exclusive). Computed ourselves —
/// rather than relying on `ratatui::widgets::Paragraph`'s built-in wrap,
/// whose exact break points aren't exposed by its public API — so the
/// very same break points can also be used by `locate_in_wrap` to map the
/// cursor's logical (row, col) onto the right visual row/col.
fn wrap_line(chars: &[char], width: usize) -> Vec<(usize, usize)> {
    let len = chars.len();
    if len == 0 {
        return vec![(0, 0)];
    }
    if width == 0 {
        return vec![(0, len)];
    }
    let mut ranges = Vec::new();
    let mut start = 0;
    while len - start > width {
        let limit = start + width;
        let mut break_at = None;
        let mut i = limit;
        while i > start {
            if chars[i] == ' ' {
                break_at = Some(i);
                break;
            }
            i -= 1;
        }
        let end = match break_at {
            Some(pos) if pos > start => pos,
            // No space anywhere in this width's worth of the line (one
            // very long word) — hard-break mid-word, same as `Paragraph`'s
            // own wrap does in that situation.
            _ => limit,
        };
        ranges.push((start, end));
        start = if chars.get(end) == Some(&' ') {
            end + 1
        } else {
            end
        };
    }
    ranges.push((start, len));
    ranges
}

/// Maps a char-index `col` within a source line onto `(local visual row,
/// visual col)` given that line's own `wrap_line` output. A `col` sitting
/// exactly on a wrap boundary belongs to the *earlier* segment only when a
/// space was actually dropped there (a soft break) — for a hard break
/// (a word too long to fit, split with no space to drop) the boundary
/// value is itself the first character of the *next* segment, so it must
/// resolve there instead, or the cursor would visually land one segment
/// too early.
fn locate_in_wrap(ranges: &[(usize, usize)], col: usize) -> (usize, usize) {
    for (i, &(s, e)) in ranges.iter().enumerate() {
        let is_last = i + 1 == ranges.len();
        let next_start = if is_last { e } else { ranges[i + 1].0 };
        let boundary_is_gap = next_start > e;
        if col < e || (col == e && (is_last || boundary_is_gap)) {
            return (i, col - s);
        }
    }
    let last = ranges.len() - 1;
    (last, col - ranges[last].0)
}

/// Smallest vertical scroll adjustment that keeps `cursor` inside a
/// `height`-row-tall viewport currently topped at `prev_top` — scrolls up
/// the instant the cursor moves above it, or down the instant it moves
/// past the bottom, and otherwise leaves the viewport exactly where it
/// was (no re-centering on every keystroke).
fn next_scroll_top(prev_top: u16, cursor: u16, height: u16) -> u16 {
    if height == 0 {
        return 0;
    }
    if cursor < prev_top {
        cursor
    } else if prev_top + height <= cursor {
        cursor + 1 - height
    } else {
        prev_top
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn wraps_on_word_boundary() {
        let line = chars("hello world foo");
        let ranges = wrap_line(&line, 11);
        assert_eq!(ranges, vec![(0, 11), (12, 15)]);
    }

    #[test]
    fn hard_breaks_a_single_long_word() {
        let line = chars("abcdefghijklmnopqrstu"); // 21 chars, no spaces
        let ranges = wrap_line(&line, 5);
        assert_eq!(ranges, vec![(0, 5), (5, 10), (10, 15), (15, 20), (20, 21)]);
    }

    #[test]
    fn empty_line_yields_one_empty_range() {
        assert_eq!(wrap_line(&[], 10), vec![(0, 0)]);
    }

    #[test]
    fn locate_maps_cursor_after_a_dropped_space() {
        // "hello world foo" wrapped at 11 -> [(0,11), (12,15)]; col 11 is
        // the dropped space itself, so it belongs to the earlier segment.
        let ranges = vec![(0, 11), (12, 15)];
        assert_eq!(locate_in_wrap(&ranges, 11), (0, 11));
        assert_eq!(locate_in_wrap(&ranges, 12), (1, 0));
    }

    #[test]
    fn locate_maps_cursor_at_a_hard_break_boundary() {
        // No space dropped here, so the boundary value belongs to the
        // *next* segment (it's that segment's first real character).
        let ranges = vec![(0, 5), (5, 10)];
        assert_eq!(locate_in_wrap(&ranges, 5), (1, 0));
    }

    #[test]
    fn scroll_follows_cursor_in_both_directions() {
        assert_eq!(next_scroll_top(0, 3, 5), 0); // cursor already visible
        assert_eq!(next_scroll_top(0, 7, 5), 3); // cursor below viewport
        assert_eq!(next_scroll_top(4, 1, 5), 1); // cursor above viewport
    }
}
