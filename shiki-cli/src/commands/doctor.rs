//! `shiki doctor` — an environment health check, independent of the normal
//! startup path (`Context::load`) so it still works (and says why) even when
//! the config is broken, which is exactly the situation someone reaching for
//! a "doctor" command is usually in.

use std::io::IsTerminal;

use anyhow::Result;
use shiki_config::Config;
use shiki_core::NotebookStore;

struct Report {
    color: bool,
    ok: u32,
    warn: u32,
    fail: u32,
}

impl Report {
    fn new() -> Self {
        Self {
            color: std::io::stdout().is_terminal(),
            ok: 0,
            warn: 0,
            fail: 0,
        }
    }

    fn pass(&mut self, label: &str, detail: impl std::fmt::Display) {
        self.ok += 1;
        self.line("\u{2713}", 32, label, detail);
    }

    fn warn(&mut self, label: &str, detail: impl std::fmt::Display) {
        self.warn += 1;
        self.line("!", 33, label, detail);
    }

    fn fail(&mut self, label: &str, detail: impl std::fmt::Display) {
        self.fail += 1;
        self.line("\u{2717}", 31, label, detail);
    }

    fn line(&self, symbol: &str, color_code: u8, label: &str, detail: impl std::fmt::Display) {
        if self.color {
            println!("\x1b[{color_code}m{symbol}\x1b[0m {label}: {detail}");
        } else {
            println!("{symbol} {label}: {detail}");
        }
    }
}

/// Whether `bin` exists somewhere on `$PATH` — a plain lookup, deliberately
/// not executing it (a `--version` probe could hang or have side effects for
/// an arbitrary user-configured editor command).
fn on_path(bin: &str) -> bool {
    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path_var).any(|dir| dir.join(bin).is_file())
}

pub fn run() -> Result<()> {
    let mut r = Report::new();
    println!(
        "shiki {} \u{2014} environment check\n",
        env!("CARGO_PKG_VERSION")
    );

    let config_path = Config::default_path();
    let config = match &config_path {
        Ok(path) if !path.exists() => {
            r.warn(
                "config",
                format!(
                    "{} \u{2014} not created yet, defaults will be used on first run",
                    path.display()
                ),
            );
            None
        }
        Ok(path) => match std::fs::read_to_string(path) {
            Ok(contents) => match Config::parse(&contents) {
                Ok(cfg) => {
                    r.pass("config", path.display());
                    Some(cfg)
                }
                Err(e) => {
                    r.fail(
                        "config",
                        format!("{} \u{2014} invalid TOML: {e}", path.display()),
                    );
                    None
                }
            },
            Err(e) => {
                r.fail(
                    "config",
                    format!("{} \u{2014} could not read: {e}", path.display()),
                );
                None
            }
        },
        Err(e) => {
            r.fail("config", e);
            None
        }
    };
    let config = config.unwrap_or_default();

    match Config::default_data_dir() {
        Ok(dir) if dir.exists() => r.pass("data dir", dir.display()),
        Ok(dir) => r.warn(
            "data dir",
            format!("{} \u{2014} not created yet", dir.display()),
        ),
        Err(e) => r.fail("data dir", e),
    }

    match Config::default_templates_dir() {
        Ok(dir) if dir.exists() => r.pass("templates dir", dir.display()),
        Ok(dir) => r.warn(
            "templates dir",
            format!("{} \u{2014} created on first run", dir.display()),
        ),
        Err(e) => r.fail("templates dir", e),
    }

    if on_path("git") {
        r.pass("git", "found on PATH");
    } else {
        r.fail(
            "git",
            "not found on PATH \u{2014} required for notebook sync/pull/push",
        );
    }

    if on_path("gh") {
        r.pass(
            "gh (GitHub CLI)",
            "found \u{2014} used by git's credential helper for HTTPS auth to private repos",
        );
    } else {
        r.warn(
            "gh (GitHub CLI)",
            "not found \u{2014} HTTPS auth to private remotes relies on your system git credential store instead",
        );
    }

    let colorterm = std::env::var("COLORTERM").unwrap_or_default();
    if colorterm.contains("truecolor") || colorterm.contains("24bit") {
        r.pass("terminal truecolor", format!("COLORTERM={colorterm}"));
    } else {
        r.warn(
            "terminal truecolor",
            "COLORTERM isn't \"truecolor\"/\"24bit\" \u{2014} themes may render with approximated \
             colors instead of exact hex (inside tmux, also check `terminal-overrides ,*:Tc` or \
             `terminal-features ,*:RGB` in .tmux.conf)",
        );
    }

    let editor_bin = config
        .general
        .editor
        .split_whitespace()
        .next()
        .unwrap_or("");
    if editor_bin.is_empty() {
        r.warn("editor", "no `general.editor` configured");
    } else if on_path(editor_bin) {
        r.pass(
            "editor",
            format!("'{}' found on PATH", config.general.editor),
        );
    } else {
        r.warn(
            "editor",
            format!(
                "'{editor_bin}' not found on PATH \u{2014} check `general.editor` in config.toml"
            ),
        );
    }

    if config.general.use_favorite_editor {
        match shiki_core::editor::detect_favorite_editor() {
            Some(fav) => r.pass("favorite editor", fav),
            None => r.warn(
                "favorite editor",
                "`use_favorite_editor` is on but none could be detected ($VISUAL/$EDITOR/xdg-mime)",
            ),
        }
    }

    if let Ok(data_dir) = Config::default_data_dir() {
        let store = NotebookStore::new(data_dir);
        match store.list() {
            Ok(notebooks) if notebooks.is_empty() => r.warn(
                "notebooks",
                "none yet \u{2014} `shiki notebook create <name>`, or `a` in the TUI",
            ),
            Ok(notebooks) => {
                let with_remote = notebooks
                    .iter()
                    .filter(|nb| shiki_core::git::remote_url(&nb.path).is_some())
                    .count();
                r.pass(
                    "notebooks",
                    format!(
                        "{} found, {with_remote} with a git remote configured",
                        notebooks.len()
                    ),
                );
            }
            Err(e) => r.fail("notebooks", e),
        }
    }

    println!("\n{} ok, {} warning(s), {} failed", r.ok, r.warn, r.fail);
    if r.fail > 0 {
        anyhow::bail!("doctor found {} problem(s) \u{2014} see above", r.fail);
    }
    Ok(())
}
