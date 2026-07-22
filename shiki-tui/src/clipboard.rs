//! Terminal clipboard via OSC 52, not a native clipboard crate.
//!
//! ratatui owns the alternate screen, but writing an OSC 52 escape sequence
//! straight to stdout still reaches the real terminal underneath it. Modern
//! terminals (kitty, iTerm2, Alacritty, WezTerm, Windows Terminal, and
//! tmux/screen with clipboard passthrough enabled) intercept this and set
//! the system clipboard — the same mechanism Yazi and Helix use, so it works
//! the same whether shiki is running locally or over SSH.

use base64::Engine;
use std::io::Write;

pub fn copy(text: &str) {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    let mut stdout = std::io::stdout();
    let _ = write!(stdout, "\x1b]52;c;{encoded}\x07");
    let _ = stdout.flush();
}
