use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::theme::Theme;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml parse error: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("toml serialize error: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("could not determine the user's config directory")]
    NoConfigDir,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct General {
    pub default_notebook: String,
    pub editor: String,
    pub daily_template: String,
    /// When true, `i` (edit inline) instead detects the OS's favorite/default
    /// text editor and opens the note there, like `E` but auto-resolved
    /// instead of using a fixed `editor` command.
    #[serde(default)]
    pub use_favorite_editor: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            default_notebook: "personal".into(),
            editor: std::env::var("EDITOR").unwrap_or_else(|_| "nvim".into()),
            daily_template: "daily".into(),
            use_favorite_editor: false,
        }
    }
}

/// Keybindings are segmented by *scope*: navigation (`hjkl`, arrows, `tab`,
/// `enter`, `?`) is hardcoded and not configurable here since it behaves the
/// same everywhere. Everything else is scoped to whichever panel has focus,
/// so the same physical key can mean different things in different panels
/// (e.g. `a` creates a notebook while NOTEBOOKS is focused, a note while
/// NOTES is focused) — each scope is its own small, independently editable
/// table below. `global` actions require the `leader` key first (press
/// leader, then the key) since they aren't tied to any one panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keybindings {
    /// Prefix key for `[keybindings.global]` actions — press this, then the
    /// action's key, e.g. `space` then `g` for global search.
    #[serde(default = "default_leader")]
    pub leader: String,
    #[serde(default = "default_quit")]
    pub quit: String,
    #[serde(default)]
    pub global: GlobalKeybindings,
    #[serde(default)]
    pub notebooks: NotebookKeybindings,
    #[serde(default)]
    pub notes: NoteKeybindings,
    #[serde(default)]
    pub preview: PreviewKeybindings,
}

fn default_leader() -> String {
    "space".into()
}

fn default_quit() -> String {
    "q".into()
}

impl Default for Keybindings {
    fn default() -> Self {
        Self {
            leader: default_leader(),
            quit: default_quit(),
            global: GlobalKeybindings::default(),
            notebooks: NotebookKeybindings::default(),
            notes: NoteKeybindings::default(),
            preview: PreviewKeybindings::default(),
        }
    }
}

/// `<leader>` + key — actions that aren't tied to a specific panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalKeybindings {
    pub theme_picker: String,
    pub global_search: String,
    pub tags_panel: String,
    /// Opens the logs modal (a scrollback of every status-bar message,
    /// including errors that already scrolled past). Field-level default so
    /// an existing `[keybindings.global]` table written before this key
    /// existed still deserializes instead of erroring on the missing field.
    #[serde(default = "default_logs_key")]
    pub logs: String,
    /// Flips `general.use_favorite_editor` on/off and persists it, without
    /// hand-editing config.toml. Field-level default for the same
    /// backward-compatibility reason as `logs`.
    #[serde(default = "default_toggle_favorite_editor_key")]
    pub toggle_favorite_editor: String,
}

impl Default for GlobalKeybindings {
    fn default() -> Self {
        Self {
            theme_picker: "c".into(),
            global_search: "g".into(),
            tags_panel: "T".into(),
            logs: default_logs_key(),
            toggle_favorite_editor: default_toggle_favorite_editor_key(),
        }
    }
}

fn default_logs_key() -> String {
    "l".into()
}

fn default_toggle_favorite_editor_key() -> String {
    "e".into()
}

/// Active only while the NOTEBOOKS panel has focus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookKeybindings {
    pub new: String,
    pub rename: String,
    pub delete: String,
    /// Stage + commit (+ push if `git.auto_push`) all changes in the notebook.
    pub sync: String,
    /// Fetch + fast-forward merge from the notebook's configured remote.
    pub pull: String,
    /// `pull` for every notebook that has a remote configured, in one go.
    pub pull_all: String,
    /// Prompts for a URL or local path and sets it as the notebook's `origin`.
    pub set_remote: String,
    /// Commits (same as `sync`) and always pushes, regardless of the
    /// resolved `auto_push` policy — the explicit "do it now" override.
    /// Field-level default so an existing `[keybindings.notebooks]` table
    /// written before this key existed still deserializes.
    #[serde(default = "default_push_key")]
    pub push: String,
}

impl Default for NotebookKeybindings {
    fn default() -> Self {
        Self {
            new: "a".into(),
            rename: "r".into(),
            delete: "d".into(),
            sync: "s".into(),
            pull: "p".into(),
            pull_all: "P".into(),
            set_remote: "R".into(),
            push: default_push_key(),
        }
    }
}

fn default_push_key() -> String {
    "u".into()
}

/// Active only while the NOTES panel has focus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteKeybindings {
    pub new: String,
    pub rename: String,
    pub delete: String,
    /// Edit in the built-in inline editor (or the favorite editor, see
    /// `general.use_favorite_editor`) — vim-style insert, not "e" for "edit".
    pub edit_inline: String,
    pub edit_external: String,
    /// Fuzzy-jump to a note by title within the current notebook.
    pub search: String,
    pub daily_note: String,
    /// Prompts for a target notebook name and moves the selected note there.
    pub move_to_notebook: String,
    /// Cycles the notes list's sort order (filename/date vs. title).
    pub sort: String,
    /// Opens the tree view — every folder and note in the notebook, fully
    /// expanded, in one scrollable overview (Enter jumps straight to a
    /// note). Field-level default so an existing `[keybindings.notes]` table
    /// written before this key existed still deserializes.
    #[serde(default = "default_tree_view_key")]
    pub tree_view: String,
    /// Shows each note's date next to its title in the list (off by
    /// default — off is the "clean" state, on is opt-in clutter). Same
    /// field-level-default backward-compatibility reasoning as `tree_view`.
    #[serde(default = "default_toggle_dates_key")]
    pub toggle_dates: String,
}

impl Default for NoteKeybindings {
    fn default() -> Self {
        Self {
            new: "a".into(),
            rename: "r".into(),
            delete: "d".into(),
            edit_inline: "i".into(),
            edit_external: "E".into(),
            search: "/".into(),
            daily_note: "t".into(),
            move_to_notebook: "m".into(),
            sort: "o".into(),
            tree_view: default_tree_view_key(),
            toggle_dates: default_toggle_dates_key(),
        }
    }
}

fn default_toggle_dates_key() -> String {
    "D".into()
}

fn default_tree_view_key() -> String {
    "T".into()
}

/// Active only while the PREVIEW panel has focus (`j`/`k`/arrows scroll
/// instead of navigating a list while here).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewKeybindings {
    pub edit_inline: String,
    pub edit_external: String,
    /// Opens the note's version history — every commit that changed it,
    /// with view + revert. Field-level default so an existing
    /// `[keybindings.preview]` table written before this key existed still
    /// deserializes.
    #[serde(default = "default_history_key")]
    pub history: String,
}

impl Default for PreviewKeybindings {
    fn default() -> Self {
        Self {
            edit_inline: "i".into(),
            edit_external: "E".into(),
            history: default_history_key(),
        }
    }
}

fn default_history_key() -> String {
    "H".into()
}

/// Theme config: `name` references a built-in theme; the optional fields
/// allow overriding individual color slots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub name: String,
    #[serde(flatten)]
    pub overrides: ThemeOverrides,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThemeOverrides {
    pub bg: Option<String>,
    pub fg: Option<String>,
    pub accent: Option<String>,
    pub selection: Option<String>,
    pub border: Option<String>,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            name: "catppuccin-mocha".into(),
            overrides: ThemeOverrides::default(),
        }
    }
}

impl ThemeConfig {
    /// Resolves the built-in theme by name and applies the configured overrides.
    pub fn resolve(&self) -> Theme {
        let mut theme = crate::themes::by_name(&self.name).unwrap_or_else(Theme::terminal_default);
        if let Some(v) = &self.overrides.bg {
            theme.bg = v.clone();
        }
        if let Some(v) = &self.overrides.fg {
            theme.fg = v.clone();
        }
        if let Some(v) = &self.overrides.accent {
            theme.accent = v.clone();
        }
        if let Some(v) = &self.overrides.selection {
            theme.selection = v.clone();
        }
        if let Some(v) = &self.overrides.border {
            theme.border = v.clone();
        }
        theme
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    pub auto_commit: bool,
    pub auto_push: bool,
    pub commit_prefix: String,
    pub remote: String,
    pub branch: String,
    pub sign_commits: bool,
    /// When on, a notebook syncs itself (commit, + push if `auto_push`)
    /// automatically after `auto_sync_every` note changes, instead of only
    /// on manual `s`. Off by default — this is opt-in per the global
    /// default, and further overridable per notebook via `[notebooks.<name>]`.
    #[serde(default)]
    pub auto_sync: bool,
    /// How many note changes (new/edited/renamed/deleted/moved) trigger an
    /// automatic sync when `auto_sync` is on.
    #[serde(default = "default_auto_sync_every")]
    pub auto_sync_every: u32,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            auto_commit: true,
            auto_push: false,
            commit_prefix: "shiki: ".into(),
            remote: "origin".into(),
            branch: "main".into(),
            sign_commits: false,
            auto_sync: false,
            auto_sync_every: default_auto_sync_every(),
        }
    }
}

fn default_auto_sync_every() -> u32 {
    5
}

/// Per-notebook override of the `[git]` sync policy — a notebook connected
/// to a private work repo might want `auto_push`, while a scratch notebook
/// with no remote at all shouldn't be forced into the same policy. Any
/// field left unset here falls back to the global `[git]` default (see
/// `Config::sync_for`), so most notebooks need no `[notebooks.<name>]`
/// table at all.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NotebookGitOverride {
    pub auto_push: Option<bool>,
    pub auto_sync: Option<bool>,
    pub auto_sync_every: Option<u32>,
}

/// `[git]` settings resolved for one specific notebook — see `Config::sync_for`.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedSync {
    pub auto_push: bool,
    pub auto_sync: bool,
    pub auto_sync_every: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub general: General,
    #[serde(default)]
    pub keybindings: Keybindings,
    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(default)]
    pub git: GitConfig,
    /// Per-notebook overrides of `[git]`, keyed by notebook name — e.g.
    /// `[notebooks.work]` with `auto_push = true`. See `NotebookGitOverride`.
    #[serde(default)]
    pub notebooks: std::collections::HashMap<String, NotebookGitOverride>,
}

impl Config {
    /// Resolves the sync policy for `notebook_name`: its `[notebooks.<name>]`
    /// entry (if any) layered on top of the global `[git]` defaults.
    pub fn sync_for(&self, notebook_name: &str) -> ResolvedSync {
        let over = self.notebooks.get(notebook_name);
        ResolvedSync {
            auto_push: over.and_then(|o| o.auto_push).unwrap_or(self.git.auto_push),
            auto_sync: over.and_then(|o| o.auto_sync).unwrap_or(self.git.auto_sync),
            auto_sync_every: over
                .and_then(|o| o.auto_sync_every)
                .unwrap_or(self.git.auto_sync_every),
        }
    }
}

impl Config {
    /// Default path: `~/.config/shiki/config.toml` (respects `$XDG_CONFIG_HOME`).
    pub fn default_path() -> Result<PathBuf> {
        let dirs = directories::ProjectDirs::from("", "", "shiki").ok_or(Error::NoConfigDir)?;
        Ok(dirs.config_dir().join("config.toml"))
    }

    pub fn default_data_dir() -> Result<PathBuf> {
        let dirs = directories::ProjectDirs::from("", "", "shiki").ok_or(Error::NoConfigDir)?;
        Ok(dirs.data_dir().to_path_buf())
    }

    pub fn default_templates_dir() -> Result<PathBuf> {
        Ok(Self::default_path()?
            .parent()
            .expect("config path always has a parent")
            .join("templates"))
    }

    /// Loads the config from `path`, or creates and saves a default config if it doesn't exist.
    pub fn load_or_init(path: &Path) -> Result<Self> {
        if path.exists() {
            let contents = std::fs::read_to_string(path)?;
            Self::parse(&contents)
        } else {
            let config = Self::default();
            config.save(path)?;
            Ok(config)
        }
    }

    /// Parses a config from its TOML text — split out from `load_or_init` so
    /// callers that already have the file's contents (e.g. `shiki doctor`,
    /// diagnosing a broken config without going through the normal
    /// load-or-create startup path) don't need a direct `toml` dependency.
    pub fn parse(contents: &str) -> Result<Self> {
        Ok(toml::from_str(contents)?)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }
}
