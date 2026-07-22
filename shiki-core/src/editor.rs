//! Detects the user's OS-level "favorite" text editor, for the
//! `use_favorite_editor` config option — an alternative to always opening
//! the built-in inline editor or a hardcoded `$EDITOR`.

use std::path::PathBuf;

/// Resolves the editor to launch, in priority order:
/// `$VISUAL` -> `$EDITOR` -> the OS's registered default text editor ->
/// `None` (caller decides the final fallback, e.g. the configured editor).
pub fn detect_favorite_editor() -> Option<String> {
    for var in ["VISUAL", "EDITOR"] {
        if let Ok(value) = std::env::var(var) {
            if !value.trim().is_empty() {
                return Some(value);
            }
        }
    }

    #[cfg(target_os = "linux")]
    if let Some(editor) = linux_default_editor() {
        return Some(editor);
    }

    #[cfg(target_os = "macos")]
    if let Some(editor) = macos_default_editor() {
        return Some(editor);
    }

    None
}

/// Asks the desktop's MIME database what opens `text/plain` (respected by
/// GNOME, KDE, and every other freedesktop-compliant environment), then
/// reads that `.desktop` file's `Exec=` line for the actual command.
#[cfg(target_os = "linux")]
fn linux_default_editor() -> Option<String> {
    let output = std::process::Command::new("xdg-mime")
        .args(["query", "default", "text/plain"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let desktop_file = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if desktop_file.is_empty() {
        return None;
    }
    desktop_exec_command(&desktop_file)
}

#[cfg(target_os = "linux")]
fn desktop_exec_command(desktop_file: &str) -> Option<String> {
    let mut dirs = Vec::new();
    if let Ok(xdg_data_home) = std::env::var("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(xdg_data_home).join("applications"));
    } else if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share/applications"));
    }
    dirs.push(PathBuf::from("/usr/local/share/applications"));
    dirs.push(PathBuf::from("/usr/share/applications"));

    for dir in dirs {
        let contents = match std::fs::read_to_string(dir.join(desktop_file)) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let exec_line = contents.lines().find_map(|l| l.strip_prefix("Exec="))?;
        // Desktop field codes (%f, %F, %u, %U, ...) are separate whitespace
        // tokens — taking just the first token gives the bare command.
        let command = exec_line.split_whitespace().next()?;
        return Some(command.to_string());
    }
    None
}

/// macOS has no single CLI equivalent to `xdg-mime` without extra tooling
/// (e.g. `duti`); `open -W -t` reliably opens (and waits on) the user's
/// default text editor GUI app, so we shell out to that instead of trying
/// to resolve a binary.
#[cfg(target_os = "macos")]
fn macos_default_editor() -> Option<String> {
    Some("open -W -t".to_string())
}

/// Splits an editor command string (e.g. `"code --wait"` or `"open -W -t"`)
/// into a program + base args, then builds a ready-to-run `Command` with
/// `path` appended as the final argument. Editor strings are single words
/// in the common case (`"nvim"`), but favorite-editor detection and some
/// user configs produce multi-word commands, so callers should always go
/// through this instead of `Command::new(editor)` directly.
pub fn command_for(editor: &str, path: &std::path::Path) -> std::process::Command {
    let mut parts = editor.split_whitespace();
    let program = parts.next().unwrap_or(editor);
    let mut command = std::process::Command::new(program);
    command.args(parts);
    command.arg(path);
    command
}
