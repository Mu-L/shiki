use std::io;
use std::time::Duration;

use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Clear, List, ListItem, ListState};
use ratatui::Terminal;
use shiki_config::{Config, Theme};
use shiki_core::search::SearchHit;
use shiki_core::{Note, Notebook, NotebookStore, SearchEngine, TagIndex};

use crate::editor::InlineEditor;
use crate::icons;
use crate::input::InputBox;
use crate::keybindings::{action_label, Action, KeyMaps};
use crate::render::{hex_to_color, panel_block};
use crate::{
    confirm, layout, panel_notebooks, panel_notes, panel_preview, panel_tags, status_bar, which,
};

/// How many rows/lines `PageUp`/`PageDown` move by, across every scrollable
/// list and the PREVIEW scroll — one consistent "big jump" step everywhere
/// instead of matching whatever's currently visible on screen (not knowable
/// from most of this code without threading the render area through).
const PAGE_STEP: isize = 10;

/// How long a status-bar message stays visible before clearing itself —
/// it's always still in `log_history` (leader+`l`) regardless, so nothing
/// is lost by clearing the footer quickly instead of leaving it there until
/// the next action happens to overwrite it.
const STATUS_MESSAGE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Edit,
    Visual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Notebooks,
    Notes,
    Preview,
}

impl Focus {
    /// Full cycle, used by `tab`.
    fn next(self) -> Self {
        match self {
            Focus::Notebooks => Focus::Notes,
            Focus::Notes => Focus::Preview,
            Focus::Preview => Focus::Notebooks,
        }
    }

    /// One level deeper (Yazi-style `l` / Right / Enter). Stays at the deepest level.
    fn forward(self) -> Self {
        match self {
            Focus::Notebooks => Focus::Notes,
            Focus::Notes | Focus::Preview => Focus::Preview,
        }
    }

    /// One level back (Yazi-style `h` / Left). Stays at the shallowest level.
    fn backward(self) -> Self {
        match self {
            Focus::Notebooks | Focus::Notes => Focus::Notebooks,
            Focus::Preview => Focus::Notes,
        }
    }
}

/// How the NOTES list is ordered; cycled by `Action::SortNotes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum NoteSort {
    #[default]
    Filename,
    TitleAz,
    DateNewest,
}

impl NoteSort {
    fn next(self) -> Self {
        match self {
            NoteSort::Filename => NoteSort::TitleAz,
            NoteSort::TitleAz => NoteSort::DateNewest,
            NoteSort::DateNewest => NoteSort::Filename,
        }
    }

    fn label(self) -> &'static str {
        match self {
            NoteSort::Filename => "filename",
            NoteSort::TitleAz => "title A-Z",
            NoteSort::DateNewest => "date (newest first)",
        }
    }
}

/// What a text-input popup is currently collecting a value for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingInput {
    NewNote,
    NewNotebook,
    NewFolder,
    RenameNote,
    RenameNotebook,
    Search,
    SetRemote,
    MoveNote,
}

impl PendingInput {
    fn title(self) -> &'static str {
        match self {
            PendingInput::NewNote => " New note ",
            PendingInput::NewNotebook => " New notebook ",
            PendingInput::NewFolder => " New folder ",
            PendingInput::RenameNote | PendingInput::RenameNotebook => " Rename ",
            PendingInput::Search => " Jump to note ",
            PendingInput::SetRemote => " Git remote (URL or local path) ",
            PendingInput::MoveNote => " Move to notebook ",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeleteTarget {
    Note,
    Notebook,
}

pub struct App {
    pub config: Config,
    pub theme: Theme,
    pub store: NotebookStore,
    pub notebooks: Vec<Notebook>,
    pub selected_notebook: usize,
    /// Subfolder names at the current path within the selected notebook,
    /// shown above `notes` in the panel. A notebook can nest folders
    /// arbitrarily deep (like `nb`) — `notes_path` is the breadcrumb.
    pub folders: Vec<String>,
    pub notes: Vec<Note>,
    /// Index into the *combined* display list (`folders` then `notes`), not
    /// into `notes` alone — use `selected_note()`/`selected_folder()` rather
    /// than indexing `notes`/`folders` with this directly.
    pub selected_note: usize,
    notes_path: Vec<String>,
    pub mode: Mode,
    pub focus: Focus,
    pub should_quit: bool,
    pub show_which_key: bool,
    pub show_tags: bool,
    pub status_message: Option<String>,
    /// When `status_message` was last set — the footer only shows it for
    /// `STATUS_MESSAGE_TIMEOUT`, after which `expire_status_message` (called
    /// once per `run()` loop iteration) clears it. It's always still in
    /// `log_history` regardless (leader+`l`), so nothing is actually lost.
    status_message_set_at: Option<std::time::Instant>,
    /// Branch/dirty/ahead-behind for the selected notebook — refreshed
    /// whenever the notebook, folder, or notes change (`refresh_git_status`).
    pub git_status: shiki_core::git::GitStatus,
    pub input: InputBox,
    pub confirm: Option<confirm::ConfirmDialog>,
    pub editor: Option<InlineEditor<'static>>,
    /// Path + editor command to launch externally, picked up by `run()`
    /// between draw calls. The editor is resolved per-invocation (either the
    /// configured `general.editor` for `E`, or the detected OS favorite for
    /// `i` when `use_favorite_editor` is on) rather than always reusing the
    /// static config value.
    pub want_external_edit: Option<(std::path::PathBuf, String)>,
    pub show_theme_picker: bool,
    pub show_global_search: bool,
    pub show_logs: bool,
    /// Every status-bar message ever set, oldest first (capped at 500) — the
    /// status bar only shows the latest one, so this is what backs the logs
    /// modal (leader+`l`) for anything that scrolled past, especially errors.
    pub log_history: Vec<LogEntry>,
    pub show_tree: bool,
    tree_rows: Vec<crate::tree::TreeRow>,
    /// Index into just the `Note` rows of `tree_rows` (folder rows are
    /// display-only and never selectable) — not a raw row index.
    tree_selected: usize,
    /// True right after the leader key is pressed, waiting for the next key
    /// to resolve against the `global` scope.
    pub leader_pending: bool,
    /// Vertical scroll offset for the preview pane (only moves while
    /// PREVIEW has focus, since there's no list to navigate there).
    pub preview_scroll: u16,
    /// Terminal area as of the last draw — reused to hit-test mouse clicks
    /// against the same popup layout that was actually rendered.
    last_frame_area: Rect,
    available_themes: Vec<Theme>,
    theme_index: usize,
    theme_picker_index: usize,
    note_sort: NoteSort,
    pending_input: Option<PendingInput>,
    pending_delete: Option<(DeleteTarget, std::path::PathBuf)>,
    global_search_pool: Vec<(Notebook, Note)>,
    global_search_input: InputBox,
    global_search_results: Vec<SearchHit>,
    global_search_selected: usize,
    search_engine: SearchEngine,
    keymaps: KeyMaps,
    logs_selected: usize,
    /// Note changes (new/edited/renamed/deleted/moved) since each
    /// notebook's last sync, keyed by notebook name — drives `auto_sync`'s
    /// `auto_sync_every` threshold (`note_changed`). Not persisted across
    /// restarts; a relaunch just starts counting from zero again.
    pending_changes: std::collections::HashMap<String, u32>,
    /// Filter query typed into the which-key modal (leader-less, `?`) —
    /// matches against the key, action label, or scope name.
    pub which_key_input: InputBox,
    pub which_key_selected: usize,
    /// The OS-detected favorite editor, resolved once at startup (not
    /// per-render — detection can shell out to `xdg-mime` on Linux, too
    /// expensive to redo every ~100ms draw tick) and reused both for the
    /// footer's editor-mode indicator and `Action::EditInline`'s dispatch,
    /// so they can never disagree with each other.
    pub favorite_editor: Option<String>,
    /// Shows each note's date next to its title in the NOTES list — off by
    /// default, toggled by `Action::ToggleDates`.
    pub show_dates: bool,
    pub show_history: bool,
    history_entries: Vec<shiki_core::git::FileRevision>,
    history_selected: usize,
    /// `Some((commit_id, content))` while viewing one historical revision's
    /// full content inside the history modal; `None` while just browsing
    /// the revision list.
    history_viewing: Option<(String, String)>,
    /// `(note path, commit id)` to revert to, staged while the `confirm`
    /// dialog is up — mirrors `pending_delete`'s pattern so `y`/`n` in
    /// `handle_confirm_key` can handle either kind of pending action.
    pending_revert: Option<(std::path::PathBuf, String)>,
    /// Cache for the footer's "{n} changes" indicator: `(note path, revision
    /// count)` for whichever note was last checked, so `run()` calling this
    /// every draw tick only actually re-walks history when the selected
    /// note has changed, not on every idle redraw.
    history_count_cache: Option<(std::path::PathBuf, usize)>,
    /// Cache for the PREVIEW panel's folder peek: `(folder's absolute path,
    /// subfolder names, note titles)` for whichever folder was last read, so
    /// `run()` calling this every draw tick only actually re-lists the
    /// directory (and re-parses each note's frontmatter) when the selected
    /// folder has changed, not on every idle redraw.
    folder_preview_cache: Option<(std::path::PathBuf, [Color; 4], Vec<Line<'static>>)>,
    /// Cache for the PREVIEW panel's note view: `(note path, [fg, accent,
    /// muted, link], formatted lines)` for whichever note was last
    /// formatted, so `run()` calling this every draw tick only re-runs
    /// `markdown_to_lines` (a full scan of the note body — real CPU cost on
    /// a large note, unlike the folder cache above this isn't I/O) when the
    /// selected note or the active theme's colors actually changed, not on
    /// every idle redraw. Colors are part of the key because the theme
    /// picker live-previews by mutating `self.theme` while browsing, and a
    /// stale-colored cache hit would show the wrong theme until the note
    /// changed.
    note_preview_cache: Option<(std::path::PathBuf, [Color; 4], Vec<Line<'static>>)>,
    pub show_update: bool,
    pub update_state: Option<UpdateState>,
    /// Set while a background thread is checking/installing, so `run()`'s
    /// poll loop (`poll_update_channel`) can pick up the result without
    /// blocking the render loop on the network call. `self_update`'s HTTP
    /// calls are synchronous/blocking, and nothing else in this app uses
    /// async — a plain `std::thread` + channel matches the rest of the
    /// codebase's synchronous poll-loop style instead of pulling in an
    /// async runtime for this one feature.
    update_rx: Option<std::sync::mpsc::Receiver<UpdateMsg>>,
    /// Set once `install_latest` succeeds — picked up by `run()` right after
    /// the next draw to spawn the freshly-installed binary and exit this
    /// process, the same "leave the alternate screen, hand off to a
    /// subprocess" shape as `want_external_edit`/`suspend_and_edit`, except
    /// this one doesn't come back (`should_quit` follows immediately after).
    pub want_relaunch: bool,
    /// The running binary's path, captured *before* `install_latest` runs.
    /// `self_replace` (used internally by `self_update`) replaces the file
    /// via unlink-then-recreate, not an atomic rename-over — so querying
    /// `std::env::current_exe()` again *after* the replace resolves to the
    /// old, now-deleted inode (`".../shiki (deleted)"` on Linux) rather than
    /// the fresh binary at that same path. The path string itself is still
    /// valid throughout (only the file's *content* changed), so capturing it
    /// early and reusing it in `relaunch_into_updated_binary` is what
    /// actually works — hit this exact bug live (`spawn FAILED: No such
    /// file or directory ... "shiki (deleted)"`) before fixing it this way.
    relaunch_exe_path: Option<std::path::PathBuf>,
}

/// State of the update modal (leader+`U`), across its whole lifecycle: a
/// cheap version check, then — only on explicit confirmation — the real
/// download+verify+install.
#[derive(Debug, Clone)]
pub enum UpdateState {
    Checking,
    Available(String),
    UpToDate,
    Downloading,
    /// Installed; `run()` will relaunch into it right after this renders once.
    Installed(String),
    Error(String),
}

/// Sent back from the background thread spawned by `open_update_check`/
/// `start_update_install` — `poll_update_channel` (called once per `run()`
/// loop iteration, same as `refresh_history_cache`) applies it to `update_state`.
enum UpdateMsg {
    CheckResult(Result<Option<String>, String>),
    InstallResult(Result<String, String>),
}

/// One recorded status-bar message, with the time it was set — shown in the
/// logs modal (leader+`l`) so an error isn't lost the moment the next status
/// update overwrites `status_message`.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub at: chrono::DateTime<chrono::Local>,
    pub message: String,
}

impl App {
    pub fn new(config: Config, store: NotebookStore) -> shiki_core::Result<Self> {
        let theme = config.theme.resolve();
        let keymaps = KeyMaps::from_config(&config.keybindings);
        let notebooks = store.list()?;
        let (folders, notes) = notebooks
            .first()
            .map(|nb| nb.list_dir(std::path::Path::new("")))
            .transpose()?
            .unwrap_or_default();
        let git_status = notebooks
            .first()
            .map(|nb| shiki_core::git::status(&nb.path, &config.git.remote))
            .unwrap_or_default();
        let available_themes = shiki_config::themes::all();
        let theme_index = available_themes
            .iter()
            .position(|t| t.name == theme.name)
            .unwrap_or(0);

        Ok(Self {
            config,
            theme,
            store,
            notebooks,
            selected_notebook: 0,
            folders,
            notes,
            selected_note: 0,
            notes_path: Vec::new(),
            mode: Mode::Normal,
            focus: Focus::Notebooks,
            should_quit: false,
            show_which_key: false,
            show_tags: false,
            status_message: None,
            status_message_set_at: None,
            git_status,
            input: InputBox::default(),
            confirm: None,
            editor: None,
            want_external_edit: None,
            show_theme_picker: false,
            show_global_search: false,
            show_logs: false,
            log_history: Vec::new(),
            show_tree: false,
            tree_rows: Vec::new(),
            tree_selected: 0,
            leader_pending: false,
            preview_scroll: 0,
            last_frame_area: Rect::default(),
            available_themes,
            theme_index,
            theme_picker_index: theme_index,
            note_sort: NoteSort::default(),
            pending_input: None,
            pending_delete: None,
            global_search_pool: Vec::new(),
            global_search_input: InputBox::default(),
            global_search_results: Vec::new(),
            global_search_selected: 0,
            search_engine: SearchEngine::new(),
            keymaps,
            logs_selected: 0,
            pending_changes: std::collections::HashMap::new(),
            which_key_input: InputBox::default(),
            which_key_selected: 0,
            favorite_editor: shiki_core::editor::detect_favorite_editor(),
            show_dates: false,
            show_history: false,
            history_entries: Vec::new(),
            history_selected: 0,
            history_viewing: None,
            pending_revert: None,
            history_count_cache: None,
            folder_preview_cache: None,
            note_preview_cache: None,
            show_update: false,
            update_state: None,
            update_rx: None,
            want_relaunch: false,
            relaunch_exe_path: None,
        })
    }

    /// Sets the status-bar message and records it in `log_history`, so an
    /// error isn't lost once the footer clears it — see the logs modal
    /// (leader+`l`) for the permanent record.
    fn set_status(&mut self, message: String) {
        self.log_history.push(LogEntry {
            at: chrono::Local::now(),
            message: message.clone(),
        });
        if self.log_history.len() > 500 {
            self.log_history.remove(0);
        }
        self.status_message = Some(message);
        self.status_message_set_at = Some(std::time::Instant::now());
    }

    /// Clears the footer's status message once it's been showing for
    /// `STATUS_MESSAGE_TIMEOUT` — called once per `run()` loop iteration.
    /// It stays in `log_history` regardless, so this only shortens how long
    /// it sits in the footer pushing other segments around, not how long
    /// it's actually retrievable.
    fn expire_status_message(&mut self) {
        if let Some(set_at) = self.status_message_set_at {
            if set_at.elapsed() >= STATUS_MESSAGE_TIMEOUT {
                self.status_message = None;
                self.status_message_set_at = None;
            }
        }
    }

    pub fn selected_notebook(&self) -> Option<&Notebook> {
        self.notebooks.get(self.selected_notebook)
    }

    /// `None` both when nothing's selected and when the current selection is
    /// a folder, not a note — check `selected_folder()` to tell those apart.
    pub fn selected_note(&self) -> Option<&Note> {
        self.selected_note
            .checked_sub(self.folders.len())
            .and_then(|idx| self.notes.get(idx))
    }

    pub fn selected_folder(&self) -> Option<&str> {
        self.folders.get(self.selected_note).map(String::as_str)
    }

    fn combined_len(&self) -> usize {
        self.folders.len() + self.notes.len()
    }

    /// Where the Notes panel currently is within the selected notebook —
    /// `""` at the notebook root, otherwise the breadcrumb joined as a path.
    pub fn notes_relative_path(&self) -> std::path::PathBuf {
        self.notes_path.iter().collect()
    }

    /// Path to the notebook's breadcrumb, for display (`"personal / projects"`).
    pub fn notes_breadcrumb(&self) -> Option<String> {
        if self.notes_path.is_empty() {
            None
        } else {
            Some(self.notes_path.join(" / "))
        }
    }

    fn apply_sort(&mut self) {
        match self.note_sort {
            NoteSort::Filename => self.notes.sort_by(|a, b| a.path.cmp(&b.path)),
            NoteSort::TitleAz => self.notes.sort_by(|a, b| {
                a.frontmatter
                    .title
                    .to_lowercase()
                    .cmp(&b.frontmatter.title.to_lowercase())
            }),
            NoteSort::DateNewest => self
                .notes
                .sort_by_key(|n| std::cmp::Reverse(n.frontmatter.date)),
        }
    }

    fn cycle_sort(&mut self) {
        let stem = self.selected_note().map(|n| n.file_stem());
        self.note_sort = self.note_sort.next();
        self.apply_sort();
        if let Some(stem) = stem {
            if let Some(idx) = self.notes.iter().position(|n| n.file_stem() == stem) {
                self.selected_note = self.folders.len() + idx;
            }
        }
        self.set_status(format!("sort: {}", self.note_sort.label()));
    }

    /// Default name for the empty-input fast path when creating a notebook:
    /// "notebook", or "notebook-2", "notebook-3", … the first one that
    /// doesn't already exist, so pressing Enter with no name repeatedly
    /// never collides or silently fails.
    fn unique_default_notebook_name(&self) -> String {
        let base = "notebook";
        if self.store.get(base).is_err() {
            return base.to_string();
        }
        let mut n = 2;
        loop {
            let candidate = format!("{base}-{n}");
            if self.store.get(&candidate).is_err() {
                return candidate;
            }
            n += 1;
        }
    }

    /// New-notebook fast path for pasting a git URL directly: derives the
    /// notebook name from the repo name, creates it, points its remote at
    /// the URL, and pulls right away.
    fn create_notebook_from_url(&mut self, url: &str) {
        let Some(name) = notebook_name_from_git_url(url) else {
            self.set_status(format!("could not derive a notebook name from '{url}'"));
            return;
        };
        let notebook = match self.store.create(&name) {
            Ok(nb) => nb,
            Err(e) => {
                self.set_status(format!("could not create '{name}': {e}"));
                return;
            }
        };
        if let Err(e) = shiki_core::git::set_remote(&notebook.path, url) {
            self.reload_notebooks();
            self.set_status(format!("created '{name}' but could not set remote: {e}"));
            return;
        }
        self.reload_notebooks();
        if let Some(idx) = self.notebooks.iter().position(|nb| nb.name == name) {
            self.selected_notebook = idx;
        }
        match shiki_core::git::pull(
            &notebook.path,
            &self.config.git.remote,
            &self.config.git.branch,
        ) {
            Ok(branch) => {
                self.reload_notes();
                if branch == self.config.git.branch {
                    self.set_status(format!("cloned '{name}' from {url}"));
                } else {
                    self.set_status(format!("cloned '{name}' from {url} (branch '{branch}')"));
                }
            }
            Err(e) => self.set_status(format!(
                "created '{name}' and set remote, but pull failed: {e}"
            )),
        }
    }

    fn reload_notebooks(&mut self) {
        self.notebooks = self.store.list().unwrap_or_default();
        if self.notebooks.is_empty() {
            self.selected_notebook = 0;
        } else {
            self.selected_notebook = self.selected_notebook.min(self.notebooks.len() - 1);
        }
        self.notes_path.clear();
        self.reload_notes();
    }

    /// Re-lists the current path (`notes_path`) within the selected
    /// notebook and resets the selection to the top — for a notebook switch
    /// or a folder change, where "resume where you were" doesn't apply.
    fn reload_notes(&mut self) {
        let relative = self.notes_relative_path();
        let (folders, notes) = self
            .selected_notebook()
            .and_then(|nb| nb.list_dir(&relative).ok())
            .unwrap_or_default();
        self.folders = folders;
        self.notes = notes;
        self.apply_sort();
        self.selected_note = 0;
        self.preview_scroll = 0;
        self.folder_preview_cache = None;
        self.note_preview_cache = None;
        self.refresh_git_status();
    }

    /// Like `reload_notes`, but keeps the same note selected (by slug) instead
    /// of resetting to the top — used after an in-place edit rather than a
    /// notebook/folder switch, so the cursor doesn't jump around underneath you.
    fn refresh_notes_preserve_selection(&mut self) {
        let stem = self.selected_note().map(|n| n.file_stem());
        let relative = self.notes_relative_path();
        let (folders, notes) = self
            .selected_notebook()
            .and_then(|nb| nb.list_dir(&relative).ok())
            .unwrap_or_default();
        self.folders = folders;
        self.notes = notes;
        self.apply_sort();
        if let Some(stem) = stem {
            if let Some(idx) = self.notes.iter().position(|n| n.file_stem() == stem) {
                self.selected_note = self.folders.len() + idx;
            }
        }
        self.folder_preview_cache = None;
        // Also covers the case this cache exists for: same note path, body
        // changed underneath it (revert, external edit, inline edit save —
        // every caller of this function). The colors-in-the-key check alone
        // wouldn't catch that, since neither the path nor the theme changed.
        self.note_preview_cache = None;
        self.refresh_git_status();
    }

    fn refresh_git_status(&mut self) {
        self.git_status = self
            .selected_notebook()
            .map(|nb| shiki_core::git::status(&nb.path, &self.config.git.remote))
            .unwrap_or_default();
    }

    fn move_selection(&mut self, delta: isize) {
        match self.focus {
            Focus::Notebooks => {
                if !self.notebooks.is_empty() {
                    self.selected_notebook =
                        shift(self.selected_notebook, delta, self.notebooks.len());
                    self.notes_path.clear();
                    self.reload_notes();
                }
            }
            Focus::Notes => {
                let len = self.combined_len();
                if len > 0 {
                    self.selected_note = shift(self.selected_note, delta, len);
                    self.preview_scroll = 0;
                }
            }
            // No list to navigate here — reuse the same keys to scroll the note instead.
            Focus::Preview => {
                let amount = delta.unsigned_abs() as u16;
                self.preview_scroll = if delta > 0 {
                    self.preview_scroll.saturating_add(amount)
                } else {
                    self.preview_scroll.saturating_sub(amount)
                };
            }
        }
    }

    /// `Home`/`Ctrl+Home`-style jump to the very first item — first
    /// notebook, first note, or the top of the note in PREVIEW.
    fn jump_to_start(&mut self) {
        match self.focus {
            Focus::Notebooks => self.move_selection(-(self.selected_notebook as isize)),
            Focus::Notes => self.move_selection(-(self.selected_note as isize)),
            Focus::Preview => self.preview_scroll = 0,
        }
    }

    /// `End`-style jump to the very last item — last notebook, last note, or
    /// (approximately) the bottom of the note in PREVIEW. The PREVIEW case
    /// clamps against the panel's actual visible height (via `layout::split`
    /// on `last_frame_area`, the same layout `draw()` uses) so it lands at
    /// the last screenful instead of overshooting into blank space the way
    /// scrolling straight to the raw source line count would; it can still
    /// slightly undershoot the true last *rendered* line for paragraphs
    /// that wrap across the panel width, since source line count doesn't
    /// account for wrapping (a `PageDown` or two closes the gap).
    fn jump_to_end(&mut self) {
        match self.focus {
            Focus::Notebooks => {
                if !self.notebooks.is_empty() {
                    let last =
                        (self.notebooks.len() - 1) as isize - self.selected_notebook as isize;
                    self.move_selection(last);
                }
            }
            Focus::Notes => {
                let len = self.combined_len();
                if len > 0 {
                    self.move_selection((len - 1) as isize - self.selected_note as isize);
                }
            }
            Focus::Preview => {
                let total_lines = self
                    .selected_note()
                    .map(|n| n.body.lines().count() as u16)
                    .unwrap_or(0);
                let content_height = layout::split(self.last_frame_area, self.focus)
                    .preview
                    .height
                    .saturating_sub(2);
                self.preview_scroll = total_lines.saturating_sub(content_height);
            }
        }
    }

    /// Yazi-style "go deeper": into a folder if one's selected in NOTES,
    /// otherwise the normal panel-to-panel forward move.
    fn navigate_forward(&mut self) {
        if self.focus == Focus::Notes {
            if let Some(folder) = self.selected_folder() {
                self.notes_path.push(folder.to_string());
                self.reload_notes();
                return;
            }
        }
        self.focus = self.focus.forward();
    }

    /// Yazi-style "go back": up one folder level if NOTES is inside one,
    /// otherwise the normal panel-to-panel backward move.
    fn navigate_backward(&mut self) {
        if self.focus == Focus::Notes && !self.notes_path.is_empty() {
            self.notes_path.pop();
            self.reload_notes();
            return;
        }
        self.focus = self.focus.backward();
    }

    fn start_input(&mut self, kind: PendingInput, prefill: String) {
        self.input.value = prefill;
        self.pending_input = Some(kind);
        self.mode = Mode::Insert;
    }

    fn open_theme_picker(&mut self) {
        self.theme_picker_index = self.theme_index;
        self.show_theme_picker = true;
    }

    /// The editor mode actually in effect right now: the resolved favorite
    /// editor's bare binary name when `use_favorite_editor` is on (falling
    /// back to the configured `general.editor` if none could be detected,
    /// matching `Action::EditInline`'s own fallback so this never claims a
    /// mode that isn't what would really happen), or `"native"` — the
    /// built-in inline editor — when it's off.
    pub fn editor_status_label(&self) -> String {
        if self.config.general.use_favorite_editor {
            let editor = self
                .favorite_editor
                .as_deref()
                .unwrap_or(&self.config.general.editor);
            editor
                .split_whitespace()
                .next()
                .unwrap_or(editor)
                .to_string()
        } else {
            "native".to_string()
        }
    }

    /// Flips `general.use_favorite_editor` and persists it immediately, so
    /// switching between the built-in editor and the OS favorite doesn't
    /// require hand-editing config.toml.
    fn toggle_favorite_editor(&mut self) {
        self.config.general.use_favorite_editor = !self.config.general.use_favorite_editor;
        if let Ok(path) = Config::default_path() {
            let _ = self.config.save(&path);
        }
        self.set_status(format!("favorite editor: {}", self.editor_status_label()));
    }

    fn handle_theme_picker_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                // Cancel: revert the live preview back to the theme that was active.
                if let Some(t) = self.available_themes.get(self.theme_index) {
                    self.theme = t.clone();
                }
                self.show_theme_picker = false;
            }
            KeyCode::Enter => {
                self.theme_index = self.theme_picker_index;
                self.config.theme.name = self.theme.name.clone();
                self.config.theme.overrides = Default::default();
                if let Ok(path) = Config::default_path() {
                    let _ = self.config.save(&path);
                }
                self.set_status(format!("theme: {}", self.theme.name));
                self.show_theme_picker = false;
            }
            KeyCode::Char('j') | KeyCode::Down => self.preview_theme_at(1),
            KeyCode::Char('k') | KeyCode::Up => self.preview_theme_at(-1),
            _ => {}
        }
    }

    /// Moves the picker cursor and immediately applies that theme so the
    /// whole UI re-themes live while browsing, before you've committed to it.
    fn preview_theme_at(&mut self, delta: isize) {
        if self.available_themes.is_empty() {
            return;
        }
        self.theme_picker_index =
            shift(self.theme_picker_index, delta, self.available_themes.len());
        if let Some(t) = self.available_themes.get(self.theme_picker_index) {
            self.theme = t.clone();
        }
    }

    fn open_logs(&mut self) {
        self.logs_selected = self.log_history.len().saturating_sub(1);
        self.show_logs = true;
    }

    fn handle_logs_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.show_logs = false,
            KeyCode::Char('j') | KeyCode::Down => {
                if self.logs_selected + 1 < self.log_history.len() {
                    self.logs_selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.logs_selected = self.logs_selected.saturating_sub(1);
            }
            KeyCode::PageDown => {
                self.logs_selected = (self.logs_selected + PAGE_STEP as usize)
                    .min(self.log_history.len().saturating_sub(1));
            }
            KeyCode::PageUp => {
                self.logs_selected = self.logs_selected.saturating_sub(PAGE_STEP as usize);
            }
            KeyCode::Home => self.logs_selected = 0,
            KeyCode::End => self.logs_selected = self.log_history.len().saturating_sub(1),
            // Copies the whole scrollback in one go — meant for pasting the
            // full context of an error somewhere else, not just one line.
            KeyCode::Char('y') | KeyCode::Char('c') => {
                let text = self
                    .log_history
                    .iter()
                    .map(|entry| format!("[{}] {}", entry.at.format("%H:%M:%S"), entry.message))
                    .collect::<Vec<_>>()
                    .join("\n");
                let count = self.log_history.len();
                crate::clipboard::copy(&text);
                self.set_status(format!("copied {count} log lines to clipboard"));
            }
            _ => {}
        }
    }

    /// Opens the update modal and kicks off a background version check
    /// against GitHub Releases — never blocks the render loop.
    fn open_update_check(&mut self) {
        self.show_update = true;
        self.update_state = Some(UpdateState::Checking);
        let (tx, rx) = std::sync::mpsc::channel();
        let current = env!("CARGO_PKG_VERSION").to_string();
        std::thread::spawn(move || {
            let result = shiki_core::update::check_latest(&current).map_err(|e| e.to_string());
            let _ = tx.send(UpdateMsg::CheckResult(result));
        });
        self.update_rx = Some(rx);
    }

    /// Only reachable once the check reported an available version — starts
    /// the real download+verify+install on a background thread.
    fn start_update_install(&mut self) {
        self.update_state = Some(UpdateState::Downloading);
        // Captured now, before the replace happens — see the field doc on
        // `relaunch_exe_path` for why this can't just be re-queried later.
        self.relaunch_exe_path = std::env::current_exe().ok();
        let (tx, rx) = std::sync::mpsc::channel();
        let current = env!("CARGO_PKG_VERSION").to_string();
        std::thread::spawn(move || {
            let result = shiki_core::update::install_latest(&current).map_err(|e| e.to_string());
            let _ = tx.send(UpdateMsg::InstallResult(result));
        });
        self.update_rx = Some(rx);
    }

    /// Non-blocking: called once per `run()` loop iteration, same as
    /// `refresh_history_cache`. Applies whatever the background thread has
    /// sent so far, if anything — `try_recv` never waits.
    fn poll_update_channel(&mut self) {
        let Some(rx) = &self.update_rx else {
            return;
        };
        match rx.try_recv() {
            Ok(UpdateMsg::CheckResult(Ok(Some(version)))) => {
                self.update_state = Some(UpdateState::Available(version));
                self.update_rx = None;
            }
            Ok(UpdateMsg::CheckResult(Ok(None))) => {
                self.update_state = Some(UpdateState::UpToDate);
                self.update_rx = None;
            }
            Ok(UpdateMsg::CheckResult(Err(e))) => {
                self.update_state = Some(UpdateState::Error(e));
                self.update_rx = None;
            }
            Ok(UpdateMsg::InstallResult(Ok(version))) => {
                self.update_state = Some(UpdateState::Installed(version));
                self.update_rx = None;
                // Automatic per the feature request — no keypress required:
                // `run()` checks this right after the next draw, so the
                // "Installed" frame renders at least once before the swap.
                self.want_relaunch = true;
            }
            Ok(UpdateMsg::InstallResult(Err(e))) => {
                self.update_state = Some(UpdateState::Error(e));
                self.update_rx = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.update_state = Some(UpdateState::Error(
                    "update check thread ended unexpectedly".into(),
                ));
                self.update_rx = None;
            }
        }
    }

    fn handle_update_key(&mut self, key: KeyEvent) {
        match &self.update_state {
            // Downloading is deliberately not escapable: closing the modal
            // wouldn't stop the background thread anyway, and re-entering
            // leader+`U` mid-download would spawn a second overlapping
            // install — simplest to just make the user wait it out.
            Some(UpdateState::Downloading) => {}
            Some(UpdateState::Available(_)) => match key.code {
                KeyCode::Enter => self.start_update_install(),
                KeyCode::Esc => {
                    self.show_update = false;
                    self.update_state = None;
                }
                _ => {}
            },
            Some(UpdateState::Installed(_)) => {
                // Any key dismisses — `run()` picks up `want_relaunch` right
                // after this same key event regardless, so this mostly just
                // avoids sitting on a stale "Installed" state if the relaunch
                // spawn itself somehow fails.
                self.show_update = false;
            }
            _ => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter) {
                    self.show_update = false;
                    self.update_state = None;
                }
            }
        }
    }

    fn open_which_key(&mut self) {
        self.which_key_input.clear();
        self.which_key_selected = 0;
        self.show_which_key = true;
    }

    /// Every keybinding entry whose key, action label, or scope name
    /// contains the current query (case-insensitive) — all of them if the
    /// query is empty. Backs both rendering and `Enter`'s execute-in-place.
    pub fn which_key_filtered_entries(&self) -> Vec<(&'static str, String, Action)> {
        let query = self.which_key_input.value.to_lowercase();
        self.keymaps
            .entries()
            .into_iter()
            .filter(|(scope, key, action)| {
                query.is_empty()
                    || key.to_lowercase().contains(&query)
                    || action_label(*action).to_lowercase().contains(&query)
                    || scope.to_lowercase().contains(&query)
            })
            .collect()
    }

    fn handle_which_key_key(&mut self, key: KeyEvent) {
        let len = self.which_key_filtered_entries().len();
        match key.code {
            KeyCode::Esc => self.show_which_key = false,
            // Executes the highlighted entry directly — the which-key modal
            // doubles as a fast command palette: type to filter, Enter to run,
            // instead of memorizing the key and closing the modal first.
            KeyCode::Enter => {
                let action = self
                    .which_key_filtered_entries()
                    .get(self.which_key_selected)
                    .map(|(_, _, action)| *action);
                if let Some(action) = action {
                    self.show_which_key = false;
                    self.handle_action(action);
                }
            }
            KeyCode::Down => {
                if self.which_key_selected + 1 < len {
                    self.which_key_selected += 1;
                }
            }
            KeyCode::Up => self.which_key_selected = self.which_key_selected.saturating_sub(1),
            KeyCode::PageDown => {
                self.which_key_selected = (self.which_key_selected + 10).min(len.saturating_sub(1))
            }
            KeyCode::PageUp => self.which_key_selected = self.which_key_selected.saturating_sub(10),
            KeyCode::Home => self.which_key_selected = 0,
            KeyCode::End => self.which_key_selected = len.saturating_sub(1),
            KeyCode::Backspace => {
                self.which_key_input.backspace();
                self.which_key_selected = 0;
            }
            KeyCode::Char(c) => {
                self.which_key_input.push(c);
                self.which_key_selected = 0;
            }
            _ => {}
        }
    }

    /// Resolves the selected note's path relative to its notebook's root —
    /// what `shiki_core::git::file_history`/`show_file_at`/`revert_file_to`
    /// need, since git works in repo-relative paths.
    fn selected_note_relative_path(&self) -> Option<(Notebook, std::path::PathBuf)> {
        let nb = self.selected_notebook()?.clone();
        let note = self.selected_note()?;
        let relative = note.path.strip_prefix(&nb.path).ok()?.to_path_buf();
        Some((nb, relative))
    }

    /// Loads the selected note's real version history (every commit that
    /// changed it) and opens the history modal.
    fn open_history(&mut self) {
        let Some((nb, relative)) = self.selected_note_relative_path() else {
            self.set_status("no note selected".into());
            return;
        };
        self.history_entries =
            shiki_core::git::file_history(&nb.path, &relative).unwrap_or_default();
        self.history_selected = 0;
        self.history_viewing = None;
        self.show_history = true;
        if self.history_entries.is_empty() {
            self.set_status("no history yet — sync (`s`) to commit this note first".into());
        }
    }

    fn handle_history_key(&mut self, key: KeyEvent) {
        if self.history_viewing.is_some() {
            match key.code {
                KeyCode::Esc => self.history_viewing = None,
                KeyCode::Char('r') => self.start_revert_selected_history(),
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.show_history = false,
            KeyCode::Enter => self.view_selected_history(),
            KeyCode::Char('r') => self.start_revert_selected_history(),
            KeyCode::Char('j') | KeyCode::Down => {
                if self.history_selected + 1 < self.history_entries.len() {
                    self.history_selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.history_selected = self.history_selected.saturating_sub(1);
            }
            KeyCode::PageDown => {
                self.history_selected = (self.history_selected + PAGE_STEP as usize)
                    .min(self.history_entries.len().saturating_sub(1));
            }
            KeyCode::PageUp => {
                self.history_selected = self.history_selected.saturating_sub(PAGE_STEP as usize);
            }
            KeyCode::Home => self.history_selected = 0,
            KeyCode::End => self.history_selected = self.history_entries.len().saturating_sub(1),
            _ => {}
        }
    }

    /// Fetches and shows the highlighted revision's full content — a plain
    /// read-only look at what the note used to say, before deciding whether
    /// to `r`evert to it.
    fn view_selected_history(&mut self) {
        let Some((nb, relative)) = self.selected_note_relative_path() else {
            return;
        };
        let Some(entry) = self.history_entries.get(self.history_selected).cloned() else {
            return;
        };
        match shiki_core::git::show_file_at(&nb.path, &entry.commit_id, &relative) {
            Ok(content) => self.history_viewing = Some((entry.commit_id, content)),
            Err(e) => self.set_status(format!("could not load revision: {e}")),
        }
    }

    /// Stages a revert of the currently highlighted (or viewed) revision
    /// behind the usual `y`/`n` confirmation, since it overwrites the
    /// note's current working content.
    fn start_revert_selected_history(&mut self) {
        let Some(note) = self.selected_note() else {
            return;
        };
        let commit_id = self
            .history_viewing
            .as_ref()
            .map(|(id, _)| id.clone())
            .or_else(|| {
                self.history_entries
                    .get(self.history_selected)
                    .map(|e| e.commit_id.clone())
            });
        let Some(commit_id) = commit_id else {
            return;
        };
        let short = commit_id.chars().take(7).collect::<String>();
        let message = format!(
            "Revert '{}' to {short} — overwrites the current content?",
            note.file_stem()
        );
        self.pending_revert = Some((note.path.clone(), commit_id));
        self.confirm = Some(confirm::ConfirmDialog::new(message));
    }

    /// Writes the reverted content back to disk and lets the normal sync
    /// flow pick it up as a pending change, same as any other edit.
    fn perform_revert(&mut self, note_path: &std::path::Path, commit_id: &str) {
        let Some(nb) = self.selected_notebook().cloned() else {
            return;
        };
        let Ok(relative) = note_path.strip_prefix(&nb.path) else {
            return;
        };
        match shiki_core::git::revert_file_to(&nb.path, commit_id, relative) {
            Ok(()) => {
                let short = commit_id.chars().take(7).collect::<String>();
                self.refresh_notes_preserve_selection();
                self.note_changed(&nb.name);
                self.set_status(format!("reverted to {short}"));
                self.show_history = false;
                self.history_viewing = None;
                self.history_count_cache = None;
            }
            Err(e) => self.set_status(format!("revert error: {e}")),
        }
    }

    /// Keeps the footer's "{n} changes" indicator up to date without
    /// re-walking the note's git history on every draw tick — only when the
    /// selected note has actually changed since the last check. Called once
    /// per `run()` loop iteration, right before drawing.
    fn refresh_history_cache(&mut self) {
        let current_path = self.selected_note().map(|n| n.path.clone());
        let Some(current_path) = current_path else {
            self.history_count_cache = None;
            return;
        };
        if self.history_count_cache.as_ref().map(|(p, _)| p) == Some(&current_path) {
            return;
        }
        let count = self
            .selected_note_relative_path()
            .and_then(|(nb, relative)| shiki_core::git::file_history(&nb.path, &relative).ok())
            .map(|revisions| revisions.len())
            .unwrap_or(0);
        self.history_count_cache = Some((current_path, count));
    }

    /// Keeps the PREVIEW panel's folder peek up to date without re-listing
    /// the directory (and re-parsing every note's frontmatter in it), *and*
    /// without re-formatting the resulting `Line`s (`format!`-ing a name or
    /// title per row) on every draw tick — only when the selected folder or
    /// the active theme's colors have actually changed since the last
    /// check. Formatting a few entries is cheap, but a folder with tens of
    /// thousands of notes made re-running it ~10x/second a real, measured
    /// CPU cost (caught by `scripts/benchmark.sh`'s `big-folder-100k`
    /// scenario) even after the underlying listing itself was cached.
    /// Called once per `run()` loop iteration, right before drawing, same
    /// spot as `refresh_history_cache`/`refresh_note_preview_cache`.
    fn refresh_folder_preview_cache(&mut self) {
        let Some(folder) = self.selected_folder().map(str::to_owned) else {
            self.folder_preview_cache = None;
            return;
        };
        let Some(nb_path) = self.selected_notebook().map(|nb| nb.path.clone()) else {
            self.folder_preview_cache = None;
            return;
        };
        let relative = self.notes_relative_path();
        let current_key = nb_path.join(&relative).join(&folder);
        let colors = [
            hex_to_color(&self.theme.fg),
            hex_to_color(&self.theme.accent),
            hex_to_color(&self.theme.muted),
            hex_to_color(&self.theme.link),
        ];
        if self
            .folder_preview_cache
            .as_ref()
            .is_some_and(|(p, c, _)| *p == current_key && *c == colors)
        {
            return;
        }
        let sub_path = relative.join(&folder);
        let (subfolders, notes) = self
            .selected_notebook()
            .and_then(|nb| nb.list_dir(&sub_path).ok())
            .unwrap_or_default();
        let note_titles: Vec<String> = notes.into_iter().map(|n| n.frontmatter.title).collect();
        let lines = panel_preview::format_folder_entries(
            &subfolders,
            &note_titles,
            colors[0],
            colors[1],
            colors[2],
        );
        self.folder_preview_cache = Some((current_key, colors, lines));
    }

    /// The cached, already-formatted lines for whichever folder is
    /// currently selected (not entered) in NOTES, for the PREVIEW panel's
    /// peek — `None` if no folder is selected or the cache hasn't caught up
    /// yet (the very next draw tick fills it in).
    pub(crate) fn folder_preview_lines(&self) -> Option<&[Line<'static>]> {
        self.folder_preview_cache
            .as_ref()
            .map(|(_, _, lines)| lines.as_slice())
    }

    /// Keeps the PREVIEW panel's note view up to date without re-running
    /// `markdown_to_lines` (a full line-by-line scan of the note body) on
    /// every draw tick — only when the selected note or the active theme's
    /// colors have actually changed since the last check. Called once per
    /// `run()` loop iteration, right before drawing, same spot as
    /// `refresh_history_cache`/`refresh_folder_preview_cache`.
    fn refresh_note_preview_cache(&mut self) {
        let Some(note) = self.selected_note() else {
            self.note_preview_cache = None;
            return;
        };
        let path = note.path.clone();
        let colors = [
            hex_to_color(&self.theme.fg),
            hex_to_color(&self.theme.accent),
            hex_to_color(&self.theme.muted),
            hex_to_color(&self.theme.link),
        ];
        if self
            .note_preview_cache
            .as_ref()
            .is_some_and(|(p, c, _)| *p == path && *c == colors)
        {
            return;
        }
        let body = note.body.clone();
        let lines =
            crate::render::markdown_to_lines(&body, colors[0], colors[1], colors[2], colors[3]);
        self.note_preview_cache = Some((path, colors, lines));
    }

    /// The cached formatted lines for whichever note is currently selected,
    /// for the PREVIEW panel — `None` if no note is selected or the cache
    /// hasn't caught up yet (the very next draw tick fills it in).
    pub(crate) fn note_preview_lines(&self) -> Option<&[Line<'static>]> {
        self.note_preview_cache
            .as_ref()
            .map(|(_, _, lines)| lines.as_slice())
    }

    /// The cached revision count for whichever note is currently selected,
    /// for the footer — `None` when no note is selected at all (vs. `Some(0)`
    /// for a note that's never been committed).
    pub fn note_revision_count(&self) -> Option<usize> {
        let note = self.selected_note()?;
        match &self.history_count_cache {
            Some((path, count)) if path == &note.path => Some(*count),
            _ => None,
        }
    }

    /// Flattens the selected notebook's whole folder tree and opens the tree
    /// view — every folder and note expanded at once, instead of navigating
    /// one level at a time.
    fn open_tree(&mut self) {
        let Some(nb) = self.selected_notebook() else {
            self.set_status("no notebook selected".into());
            return;
        };
        self.tree_rows = crate::tree::build(nb);
        self.tree_selected = 0;
        self.show_tree = true;
    }

    /// How many `Note` rows are in `tree_rows` — the bound for `tree_selected`.
    fn tree_note_count(&self) -> usize {
        self.tree_rows
            .iter()
            .filter(|r| matches!(r, crate::tree::TreeRow::Note { .. }))
            .count()
    }

    /// The row index (into `tree_rows`, folders included) of the
    /// `tree_selected`-th note — what `ListState::select` needs to highlight
    /// the right visual row, since folder headers are interspersed.
    fn tree_selected_row(&self) -> Option<usize> {
        self.tree_rows
            .iter()
            .enumerate()
            .filter(|(_, r)| matches!(r, crate::tree::TreeRow::Note { .. }))
            .nth(self.tree_selected)
            .map(|(row, _)| row)
    }

    fn handle_tree_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.show_tree = false,
            KeyCode::Char('j') | KeyCode::Down => {
                if self.tree_selected + 1 < self.tree_note_count() {
                    self.tree_selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.tree_selected = self.tree_selected.saturating_sub(1);
            }
            KeyCode::PageDown => {
                self.tree_selected = (self.tree_selected + PAGE_STEP as usize)
                    .min(self.tree_note_count().saturating_sub(1));
            }
            KeyCode::PageUp => {
                self.tree_selected = self.tree_selected.saturating_sub(PAGE_STEP as usize);
            }
            KeyCode::Home => self.tree_selected = 0,
            KeyCode::End => self.tree_selected = self.tree_note_count().saturating_sub(1),
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => self.jump_to_tree_selection(),
            _ => {}
        }
    }

    /// The deep link: points the breadcrumb at the selected note's folder,
    /// reloads, selects it, and focuses the preview so it's ready to read.
    fn jump_to_tree_selection(&mut self) {
        let Some(row) = self.tree_selected_row() else {
            self.show_tree = false;
            return;
        };
        let crate::tree::TreeRow::Note { note, .. } = &self.tree_rows[row] else {
            self.show_tree = false;
            return;
        };
        let note_path = note.path.clone();
        let title = note.frontmatter.title.clone();
        let notebook_path = self.selected_notebook().map(|nb| nb.path.clone());
        if let Some(notebook_path) = notebook_path {
            self.notes_path = relative_folder(&note_path, &notebook_path);
        }
        self.reload_notes();
        if let Some(idx) = self.notes.iter().position(|n| n.path == note_path) {
            self.selected_note = self.folders.len() + idx;
        }
        self.focus = Focus::Preview;
        self.set_status(format!("opened '{title}'"));
        self.show_tree = false;
    }

    /// Loads every note from every notebook and opens the global search modal.
    fn open_global_search(&mut self) {
        self.global_search_pool = self.store.all_notes().unwrap_or_default();
        self.global_search_input = InputBox::default();
        self.refresh_global_search();
        self.show_global_search = true;
    }

    /// Re-scores `global_search_pool` against the current query (title +
    /// body + notebook name, so this behaves like a grep across all notes,
    /// not just a title filter).
    fn refresh_global_search(&mut self) {
        let query = self.global_search_input.value.clone();
        let haystacks: Vec<String> = self
            .global_search_pool
            .iter()
            .map(|(nb, note)| format!("{} {} {}", nb.name, note.frontmatter.title, note.body))
            .collect();
        let haystack_refs: Vec<&str> = haystacks.iter().map(String::as_str).collect();
        let mut hits = self.search_engine.search_text(&query, &haystack_refs);
        hits.truncate(30);
        self.global_search_results = hits;
        self.global_search_selected = 0;
    }

    fn handle_global_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.show_global_search = false,
            KeyCode::Enter => {
                if let Some(hit) = self
                    .global_search_results
                    .get(self.global_search_selected)
                    .copied()
                {
                    self.jump_to_global_hit(hit.index);
                }
            }
            KeyCode::Down => {
                if self.global_search_selected + 1 < self.global_search_results.len() {
                    self.global_search_selected += 1;
                }
            }
            KeyCode::Up => {
                self.global_search_selected = self.global_search_selected.saturating_sub(1)
            }
            KeyCode::PageDown => {
                self.global_search_selected = (self.global_search_selected + PAGE_STEP as usize)
                    .min(self.global_search_results.len().saturating_sub(1));
            }
            KeyCode::PageUp => {
                self.global_search_selected = self
                    .global_search_selected
                    .saturating_sub(PAGE_STEP as usize);
            }
            KeyCode::Home => self.global_search_selected = 0,
            KeyCode::End => {
                self.global_search_selected = self.global_search_results.len().saturating_sub(1)
            }
            KeyCode::Backspace => {
                self.global_search_input.backspace();
                self.refresh_global_search();
            }
            KeyCode::Char(c) => {
                self.global_search_input.push(c);
                self.refresh_global_search();
            }
            _ => {}
        }
    }

    /// The deep link: switches to the hit's notebook, selects the note, and
    /// focuses the preview so you land reading it immediately.
    fn jump_to_global_hit(&mut self, pool_index: usize) {
        if let Some((nb, note)) = self.global_search_pool.get(pool_index).cloned() {
            if let Some(nb_idx) = self.notebooks.iter().position(|n| n.name == nb.name) {
                self.selected_notebook = nb_idx;
            }
            // The hit might be nested inside a subfolder of its notebook —
            // point the breadcrumb at it before reloading so it's visible.
            self.notes_path = relative_folder(&note.path, &nb.path);
            self.reload_notes();
            if let Some(note_idx) = self.notes.iter().position(|n| n.path == note.path) {
                self.selected_note = self.folders.len() + note_idx;
            }
            self.focus = Focus::Preview;
            self.set_status(format!("opened '{}'", note.frontmatter.title));
        }
        self.show_global_search = false;
    }

    /// Hit-tests a mouse click against the global search results list, using
    /// the same layout math `draw` used to render it last frame.
    fn global_search_hit_at(&self, col: u16, row: u16) -> Option<usize> {
        let popup_area = global_search_popup_area(self.last_frame_area);
        let (_, list_area) = global_search_layout(popup_area);
        let inner_left = list_area.x + 1;
        let inner_right = list_area.x + list_area.width.saturating_sub(1);
        let inner_top = list_area.y + 1;
        let inner_bottom = list_area.y + list_area.height.saturating_sub(1);
        if col < inner_left || col >= inner_right || row < inner_top || row >= inner_bottom {
            return None;
        }
        let index = (row - inner_top) as usize;
        (index < self.global_search_results.len()).then_some(index)
    }

    pub fn on_mouse(&mut self, mouse: MouseEvent) {
        if !self.show_global_search {
            return;
        }
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            if let Some(index) = self.global_search_hit_at(mouse.column, mouse.row) {
                if let Some(hit) = self.global_search_results.get(index).copied() {
                    self.jump_to_global_hit(hit.index);
                }
            }
        }
    }

    fn start_delete_notebook(&mut self) {
        if let Some(nb) = self.selected_notebook() {
            let message = format!("Delete notebook '{}' and all its notes?", nb.name);
            self.pending_delete = Some((DeleteTarget::Notebook, nb.path.clone()));
            self.confirm = Some(confirm::ConfirmDialog::new(message));
        }
    }

    fn start_delete_note(&mut self) {
        if let Some(note) = self.selected_note() {
            let message = format!("Delete note '{}'?", note.file_stem());
            self.pending_delete = Some((DeleteTarget::Note, note.path.clone()));
            self.confirm = Some(confirm::ConfirmDialog::new(message));
        }
    }

    fn start_rename_notebook(&mut self) {
        if let Some(name) = self.selected_notebook().map(|nb| nb.name.clone()) {
            self.start_input(PendingInput::RenameNotebook, name);
        }
    }

    fn start_rename_note(&mut self) {
        if let Some(title) = self.selected_note().map(|n| n.frontmatter.title.clone()) {
            self.start_input(PendingInput::RenameNote, title);
        }
    }

    fn start_move_note(&mut self) {
        if self.selected_note().is_some() {
            self.start_input(PendingInput::MoveNote, String::new());
        } else {
            self.set_status("no note selected".into());
        }
    }

    fn move_selected_note_to(&mut self, target_name: &str) {
        let Some(source_nb) = self.selected_notebook().cloned() else {
            return;
        };
        let Some(note) = self.selected_note().cloned() else {
            return;
        };
        if target_name == source_nb.name {
            self.set_status("already in that notebook".into());
            return;
        }
        match self.store.get(target_name) {
            Ok(target_nb) => {
                let mut moved = note.clone();
                moved.frontmatter.notebook = target_nb.name.clone();
                moved.path = target_nb.note_path(&note.file_stem());
                match moved.save() {
                    Ok(()) => {
                        let _ = source_nb.delete_note_at(&note.path);
                        self.reload_notes();
                        self.note_changed(&source_nb.name);
                        self.note_changed(target_name);
                        self.set_status(format!(
                            "moved '{}' to '{target_name}'",
                            note.frontmatter.title
                        ));
                    }
                    Err(e) => self.set_status(format!("could not move note: {e}")),
                }
            }
            Err(_) => self.set_status(format!("notebook '{target_name}' not found")),
        }
    }

    fn start_set_remote(&mut self) {
        if self.selected_notebook().is_none() {
            self.set_status("no notebook selected".into());
            return;
        }
        let prefill = self
            .selected_notebook()
            .and_then(|nb| shiki_core::git::remote_url(&nb.path))
            .unwrap_or_default();
        self.start_input(PendingInput::SetRemote, prefill);
    }

    fn sync_notebook(&mut self) {
        let Some(nb) = self.selected_notebook().cloned() else {
            self.set_status("no notebook selected".into());
            return;
        };
        let message = self.run_sync(&nb, false);
        self.pending_changes.insert(nb.name.clone(), 0);
        self.set_status(message);
        self.refresh_git_status();
    }

    /// Commits (message auto-built from the diff, naming the actual files
    /// when there are only a few, e.g. "shiki: added (First note.md)") and
    /// pushes if `force_push` is set or this notebook's resolved policy
    /// (`Config::sync_for` — global `[git]`, or its `[notebooks.<name>]`
    /// override) has `auto_push` on. Shared by manual `s` (`force_push:
    /// false` — respects the configured policy), manual `u` (`force_push:
    /// true` — always pushes right now regardless of policy), and the
    /// automatic every-N-changes trigger (`note_changed`, `force_push:
    /// false`). Every step is reported explicitly (commit outcome, then push
    /// outcome including remote-side verification) rather than a terse
    /// "done" — push failures (no internet, auth, rejected by the remote,
    /// etc.) are surfaced, never panic: the commit already succeeded either
    /// way, so nothing pending is lost, and the next attempt just retries
    /// the push.
    fn run_sync(&mut self, nb: &Notebook, force_push: bool) -> String {
        let sync = self.config.sync_for(&nb.name);
        let summary =
            shiki_core::git::diff_summary(&nb.path).unwrap_or_else(|_| "changes".to_string());
        let message = format!("{}{summary}", self.config.git.commit_prefix);
        let mut parts = Vec::new();
        match shiki_core::git::commit_all(&nb.path, &message) {
            Ok(true) => {
                parts.push(format!("committed: {summary}"));
                // A new commit may have changed the currently-previewed
                // note's revision count — force the footer's cache to
                // recompute instead of showing a stale number.
                self.history_count_cache = None;
            }
            Ok(false) => parts.push("no changes to commit".to_string()),
            Err(e) => parts.push(format!("commit error: {e}")),
        }
        if force_push || sync.auto_push {
            if shiki_core::git::remote_url(&nb.path).is_none() {
                parts.push("no remote configured (press R, then s)".to_string());
            } else {
                match shiki_core::git::push(&nb.path, &self.config.git.remote) {
                    Ok(()) => parts.push("pushed and confirmed by remote".to_string()),
                    Err(e) => parts.push(format!("push error: {e}")),
                }
            }
        }
        parts.join("; ")
    }

    /// Commits (same as `s`) and always pushes, regardless of the resolved
    /// `auto_push` policy — the explicit "sync right now" override, for
    /// pushing without waiting on `auto_sync`'s threshold or turning
    /// `auto_push` on globally.
    fn push_notebook(&mut self) {
        let Some(nb) = self.selected_notebook().cloned() else {
            self.set_status("no notebook selected".into());
            return;
        };
        let message = self.run_sync(&nb, true);
        self.pending_changes.insert(nb.name.clone(), 0);
        self.set_status(format!("'{}': {message}", nb.name));
        self.refresh_git_status();
    }

    /// Call after any note create/edit/rename/delete/move: bumps
    /// `notebook_name`'s pending-change count and, if `auto_sync` is on for
    /// it (`Config::sync_for`) and the count reaches `auto_sync_every`,
    /// syncs immediately and resets the counter. A no-op notebook whose
    /// policy has `auto_sync` off, so this is cheap to call unconditionally.
    fn note_changed(&mut self, notebook_name: &str) {
        let sync = self.config.sync_for(notebook_name);
        if !sync.auto_sync {
            return;
        }
        let count = self
            .pending_changes
            .entry(notebook_name.to_string())
            .or_insert(0);
        *count += 1;
        let reached = *count >= sync.auto_sync_every.max(1);
        if !reached {
            return;
        }
        self.pending_changes.insert(notebook_name.to_string(), 0);

        let Some(nb) = self
            .notebooks
            .iter()
            .find(|nb| nb.name == notebook_name)
            .cloned()
        else {
            return;
        };
        let message = self.run_sync(&nb, false);
        self.set_status(format!("auto-sync '{notebook_name}': {message}"));
        if self.selected_notebook().map(|n| n.name.as_str()) == Some(notebook_name) {
            self.refresh_git_status();
        }
    }

    fn pull_notebook(&mut self) {
        let Some(nb) = self.selected_notebook().cloned() else {
            self.set_status("no notebook selected".into());
            return;
        };
        // Check upfront rather than letting git2 fail with a generic
        // "remote 'origin' does not exist" — that error doesn't say which
        // notebook it's about, and is easy to hit by accident: `p` pulls
        // whichever notebook is currently selected, which after switching
        // notebooks or relaunching may not be the one a remote was set on.
        if shiki_core::git::remote_url(&nb.path).is_none() {
            self.set_status(format!(
                "'{}' has no remote configured — press R to set one, then p to pull",
                nb.name
            ));
            return;
        }
        match shiki_core::git::pull(&nb.path, &self.config.git.remote, &self.config.git.branch) {
            Ok(branch) => {
                let note = if branch == self.config.git.branch {
                    format!("pulled '{}'", nb.name)
                } else {
                    format!(
                        "pulled '{}' (remote's default branch is '{branch}', not '{}')",
                        nb.name, self.config.git.branch
                    )
                };
                self.set_status(note);
                self.reload_notes();
            }
            Err(e) => self.set_status(format!("pull error ('{}'): {e}", nb.name)),
        }
    }

    fn pull_all_notebooks(&mut self) {
        let remote = self.config.git.remote.clone();
        let branch = self.config.git.branch.clone();
        let (mut ok, mut failed) = (0u32, 0u32);
        for nb in self.notebooks.clone() {
            match shiki_core::git::pull(&nb.path, &remote, &branch) {
                Ok(_) => ok += 1,
                Err(_) => failed += 1,
            }
        }
        self.set_status(format!("pull all: {ok} ok, {failed} failed"));
        self.reload_notes();
    }

    fn create_daily_note(&mut self) {
        let Some(nb) = self.selected_notebook().cloned() else {
            self.set_status("no notebook selected".into());
            return;
        };
        let templates_dir = match Config::default_templates_dir() {
            Ok(dir) => dir,
            Err(e) => {
                self.set_status(format!("daily note error: {e}"));
                return;
            }
        };
        let today = chrono::Local::now().date_naive();
        match shiki_core::daily::create_or_open(&nb, today, &templates_dir) {
            Ok(note) => {
                // Daily notes always live at the notebook root — jump the
                // breadcrumb back there so the new note is visible.
                self.notes_path.clear();
                self.reload_notes();
                if let Some(idx) = self.notes.iter().position(|n| n.path == note.path) {
                    self.selected_note = self.folders.len() + idx;
                }
                self.focus = Focus::Notes;
                self.note_changed(&nb.name);
                self.set_status(format!("daily note: {}", note.frontmatter.title));
            }
            Err(e) => self.set_status(format!("daily note error: {e}")),
        }
    }

    fn start_edit_inline(&mut self) {
        if let Some(note) = self.selected_note() {
            let mut editor = InlineEditor::from_contents(&note.body);
            let title = format!(" {}  Editing: {} ", icons::PENCIL, note.frontmatter.title);
            editor.textarea.set_block(panel_block(
                ratatui::text::Line::from(title),
                true,
                &self.theme,
            ));
            editor.textarea.set_style(
                ratatui::style::Style::default()
                    .fg(hex_to_color(&self.theme.fg))
                    .bg(hex_to_color(&self.theme.bg)),
            );
            editor.textarea.set_cursor_line_style(
                ratatui::style::Style::default().fg(hex_to_color(&self.theme.fg)),
            );
            self.editor = Some(editor);
            self.mode = Mode::Edit;
        }
    }

    fn save_and_exit_edit(&mut self) {
        let note = self.selected_note().cloned();
        let editor = self.editor.take();
        if let (Some(editor), Some(mut note)) = (editor, note) {
            note.body = editor.contents();
            let _ = note.save();
            self.note_changed(&note.frontmatter.notebook);
        }
        self.mode = Mode::Normal;
        self.refresh_notes_preserve_selection();
    }

    fn handle_action(&mut self, action: Action) {
        match action {
            Action::ThemePicker => self.open_theme_picker(),
            Action::GlobalSearch => self.open_global_search(),
            Action::ToggleTags => self.show_tags = !self.show_tags,
            Action::ShowLogs => self.open_logs(),
            Action::CheckForUpdate => self.open_update_check(),

            Action::NewNotebook => self.start_input(PendingInput::NewNotebook, String::new()),
            Action::RenameNotebook => self.start_rename_notebook(),
            Action::DeleteNotebook => self.start_delete_notebook(),
            Action::SyncNotebook => self.sync_notebook(),
            Action::PushNotebook => self.push_notebook(),
            Action::PullNotebook => self.pull_notebook(),
            Action::PullAllNotebooks => self.pull_all_notebooks(),
            Action::SetRemote => self.start_set_remote(),

            Action::NewNote => self.start_input(PendingInput::NewNote, String::new()),
            Action::NewFolder => self.start_input(PendingInput::NewFolder, String::new()),
            Action::RenameNote => self.start_rename_note(),
            Action::DeleteNote => self.start_delete_note(),
            Action::JumpSearch => self.start_input(PendingInput::Search, String::new()),
            Action::DailyNote => self.create_daily_note(),
            Action::MoveNote => self.start_move_note(),
            Action::SortNotes => self.cycle_sort(),
            Action::ToggleTreeView => self.open_tree(),
            Action::ToggleDates => {
                self.show_dates = !self.show_dates;
                self.set_status(format!(
                    "note dates: {}",
                    if self.show_dates { "on" } else { "off" }
                ));
            }
            Action::ShowHistory => self.open_history(),
            Action::ToggleFavoriteEditor => self.toggle_favorite_editor(),

            Action::EditInline => {
                if self.config.general.use_favorite_editor {
                    if let Some(note) = self.selected_note() {
                        let editor = self
                            .favorite_editor
                            .clone()
                            .unwrap_or_else(|| self.config.general.editor.clone());
                        self.want_external_edit = Some((note.path.clone(), editor));
                    }
                } else {
                    self.start_edit_inline();
                }
            }
            Action::EditExternal => {
                if let Some(note) = self.selected_note() {
                    self.want_external_edit =
                        Some((note.path.clone(), self.config.general.editor.clone()));
                }
            }
        }
    }

    fn confirm_input(&mut self) {
        let value = self.input.value.trim().to_string();
        let kind = self.pending_input.take();
        match kind {
            Some(PendingInput::NewNote) => {
                // Enter on an empty title doesn't cancel — it's the fast path:
                // stamp today's date as the title and go straight to writing.
                let title = if value.is_empty() {
                    chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()
                } else {
                    value
                };
                match self.selected_notebook().cloned() {
                    Some(nb) => match nb.create_note_in(&self.notes_relative_path(), &title, "") {
                        Ok(note) => {
                            self.reload_notes();
                            self.focus = Focus::Notes;
                            if let Some(idx) = self.notes.iter().position(|n| n.path == note.path) {
                                self.selected_note = self.folders.len() + idx;
                            }
                            self.set_status(format!("created '{title}'"));
                            // Drop straight into the inline editor — a blank
                            // note with just a title isn't useful on its own.
                            self.start_edit_inline();
                            return;
                        }
                        Err(e) => self.set_status(format!("could not create note: {e}")),
                    },
                    None => self.set_status("create a notebook first".into()),
                }
            }
            Some(PendingInput::NewFolder) => {
                // Unlike NewNote, an empty name has no sensible default (a
                // timestamp makes a fine note title but a confusing folder
                // name) — cancel instead of creating one.
                if value.is_empty() {
                    self.set_status("new folder cancelled (name can't be empty)".into());
                } else {
                    match self.selected_notebook().cloned() {
                        Some(nb) => {
                            match nb.create_folder_in(&self.notes_relative_path(), &value) {
                                Ok(_) => {
                                    self.reload_notes();
                                    if let Some(idx) = self.folders.iter().position(|f| f == &value)
                                    {
                                        self.selected_note = idx;
                                    }
                                    self.set_status(format!("created folder '{value}'"));
                                }
                                Err(e) => self.set_status(format!("could not create folder: {e}")),
                            }
                        }
                        None => self.set_status("create a notebook first".into()),
                    }
                }
            }
            Some(PendingInput::NewNotebook) => {
                // Pasting a URL is the "import someone else's repo" fast
                // path: derive the name from the repo, create, set the
                // remote, and pull, instead of new notebook + name + `R` +
                // URL + `p` as four separate steps.
                if !value.is_empty() && looks_like_git_url(&value) {
                    self.create_notebook_from_url(&value);
                } else {
                    // Same fast path as NewNote: Enter on an empty name
                    // doesn't cancel, it just picks a default so something
                    // visibly appears instead of the modal silently closing.
                    let name = if value.is_empty() {
                        self.unique_default_notebook_name()
                    } else {
                        value
                    };
                    match self.store.create(&name) {
                        Ok(_) => {
                            self.reload_notebooks();
                            if let Some(idx) = self.notebooks.iter().position(|nb| nb.name == name)
                            {
                                self.selected_notebook = idx;
                                self.reload_notes();
                            }
                            self.set_status(format!("notebook '{name}' created"));
                        }
                        Err(e) => self.set_status(format!("could not create: {e}")),
                    }
                }
            }
            Some(PendingInput::RenameNote) => {
                if value.is_empty() {
                    self.set_status("rename cancelled (title can't be empty)".into());
                } else if let (Some(nb), Some(path)) = (
                    self.selected_notebook().cloned(),
                    self.selected_note().map(|n| n.path.clone()),
                ) {
                    match nb.rename_note_at(&path, &value) {
                        Ok(_) => {
                            self.refresh_notes_preserve_selection();
                            self.note_changed(&nb.name);
                            self.set_status(format!("renamed to '{value}'"));
                        }
                        Err(e) => self.set_status(format!("could not rename: {e}")),
                    }
                }
            }
            Some(PendingInput::RenameNotebook) => {
                if value.is_empty() {
                    self.set_status("rename cancelled (name can't be empty)".into());
                } else if let Some(old_name) = self.selected_notebook().map(|nb| nb.name.clone()) {
                    match self.store.rename(&old_name, &value) {
                        Ok(_) => {
                            self.reload_notebooks();
                            self.set_status(format!("renamed to '{value}'"));
                        }
                        Err(e) => self.set_status(format!("could not rename: {e}")),
                    }
                }
            }
            Some(PendingInput::Search) => {
                if value.is_empty() {
                    self.set_status("jump cancelled".into());
                } else {
                    // Searches the whole notebook (any folder depth), not
                    // just the folder currently open, then hops there.
                    let pool = self
                        .selected_notebook()
                        .and_then(|nb| nb.all_notes_recursive().ok())
                        .unwrap_or_default();
                    let hits = self.search_engine.search(&value, &pool);
                    if let Some(hit) = hits.first().map(|h| pool[h.index].clone()) {
                        if let Some(nb) = self.selected_notebook().cloned() {
                            self.notes_path = relative_folder(&hit.path, &nb.path);
                        }
                        self.reload_notes();
                        if let Some(idx) = self.notes.iter().position(|n| n.path == hit.path) {
                            self.selected_note = self.folders.len() + idx;
                        }
                        self.preview_scroll = 0;
                        self.focus = Focus::Notes;
                        self.set_status(format!("jumped to '{value}'"));
                    } else {
                        self.set_status(format!("no match for '{value}'"));
                    }
                }
            }
            Some(PendingInput::SetRemote) => {
                if value.is_empty() {
                    self.set_status("remote cancelled (empty)".into());
                } else if let Some(nb) = self.selected_notebook().cloned() {
                    match shiki_core::git::set_remote(&nb.path, &value) {
                        Ok(()) => self.set_status(format!("remote set to '{value}'")),
                        Err(e) => self.set_status(format!("could not set remote: {e}")),
                    }
                }
            }
            Some(PendingInput::MoveNote) => {
                if value.is_empty() {
                    self.set_status("move cancelled (empty)".into());
                } else {
                    self.move_selected_note_to(&value);
                }
            }
            None => {}
        }
        self.mode = Mode::Normal;
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some((target, path)) = self.pending_delete.take() {
                    match target {
                        DeleteTarget::Note => {
                            let name = path
                                .file_stem()
                                .map(|s| s.to_string_lossy().to_string())
                                .unwrap_or_default();
                            if let Some(nb) = self.selected_notebook().cloned() {
                                let _ = nb.delete_note_at(&path);
                                self.note_changed(&nb.name);
                            }
                            self.reload_notes();
                            self.set_status(format!("deleted '{name}'"));
                        }
                        DeleteTarget::Notebook => {
                            let name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            let _ = self.store.delete(&name);
                            self.reload_notebooks();
                            self.set_status(format!("notebook '{name}' deleted"));
                        }
                    }
                } else if let Some((note_path, commit_id)) = self.pending_revert.take() {
                    self.perform_revert(&note_path, &commit_id);
                }
            }
            _ => {
                self.pending_delete = None;
                self.pending_revert = None;
            }
        }
        self.confirm = None;
    }

    fn handle_insert_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.pending_input = None;
                self.mode = Mode::Normal;
            }
            KeyCode::Enter => self.confirm_input(),
            KeyCode::Backspace => self.input.backspace(),
            KeyCode::Char(c) => self.input.push(c),
            _ => {}
        }
    }

    fn handle_edit_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.save_and_exit_edit(),
            _ => {
                if let Some(editor) = &mut self.editor {
                    editor.textarea.input(key);
                }
            }
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        if self.leader_pending {
            self.leader_pending = false;
            if key.code != KeyCode::Esc {
                match self.keymaps.resolve_global(key.code) {
                    Some(action) => self.handle_action(action),
                    None => self.set_status("unknown leader shortcut".into()),
                }
            }
            return;
        }

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(-1),
            KeyCode::PageDown => self.move_selection(PAGE_STEP),
            KeyCode::PageUp => self.move_selection(-PAGE_STEP),
            KeyCode::Home => self.jump_to_start(),
            KeyCode::End => self.jump_to_end(),
            // Yazi-style: right/l/enter opens (a folder, or one level deeper
            // panel-wise), left/h backs out (up a folder, then a panel).
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => self.navigate_forward(),
            KeyCode::Char('h') | KeyCode::Left => self.navigate_backward(),
            KeyCode::Tab => self.focus = self.focus.next(),
            KeyCode::Char('?') => self.open_which_key(),
            code if self.keymaps.is_quit(code) => self.should_quit = true,
            code if self.keymaps.is_leader(code) => self.leader_pending = true,
            _ => {
                if let Some(action) = self.keymaps.resolve_scoped(self.focus, key.code) {
                    self.handle_action(action);
                } else if let Some(action) = self.keymaps.resolve_scoped(Focus::Notebooks, key.code)
                {
                    // Git actions operate on whichever notebook is
                    // *selected*, not on whichever panel is *focused* — `u`
                    // (push) while reading a note in PREVIEW should still
                    // push that note's notebook instead of silently doing
                    // nothing just because NOTEBOOKS isn't the active panel.
                    // Deliberately NOT NewNotebook/RenameNotebook/
                    // DeleteNotebook here: those share letters with
                    // Notes-scope actions (`a`/`r`/`d`), and falling back to
                    // them from the wrong panel would be a dangerous
                    // accidental-notebook-deletion footgun.
                    if is_notebook_git_action(action) {
                        self.handle_action(action);
                    }
                }
            }
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        // Checked first, ahead of every other modal: a revert confirmation
        // can be opened *from inside* the history modal (confirm-over-modal),
        // and confirm must intercept `y`/`n` in that case rather than the
        // modal underneath it swallowing the keypress first.
        if self.confirm.is_some() {
            self.handle_confirm_key(key);
            return;
        }
        if self.show_which_key {
            self.handle_which_key_key(key);
            return;
        }
        if self.show_tags {
            self.show_tags = false;
            return;
        }
        if self.show_theme_picker {
            self.handle_theme_picker_key(key);
            return;
        }
        if self.show_global_search {
            self.handle_global_search_key(key);
            return;
        }
        if self.show_logs {
            self.handle_logs_key(key);
            return;
        }
        if self.show_update {
            self.handle_update_key(key);
            return;
        }
        if self.show_tree {
            self.handle_tree_key(key);
            return;
        }
        if self.show_history {
            self.handle_history_key(key);
            return;
        }
        match self.mode {
            Mode::Insert => self.handle_insert_key(key),
            Mode::Edit => self.handle_edit_key(key),
            Mode::Normal | Mode::Visual => self.handle_normal_key(key),
        }
    }

    pub fn keymaps(&self) -> &KeyMaps {
        &self.keymaps
    }
}

/// Whether the new-notebook input looks like something to clone rather than
/// a plain name — covers the schemes/forms an `origin` remote can actually
/// take (`set_remote` accepts the same set): `https://`, `git@host:...`
/// (SSH scp-like syntax), `ssh://`, `git://`.
/// Notebook actions that operate on "the selected notebook" as a concept
/// rather than "the NOTEBOOKS panel" — safe to reach from any focus (see the
/// fallback in `handle_normal_key`). Intentionally excludes New/Rename/
/// Delete-notebook, which share letters with Notes-scope actions.
fn is_notebook_git_action(action: Action) -> bool {
    matches!(
        action,
        Action::SyncNotebook
            | Action::PullNotebook
            | Action::PullAllNotebooks
            | Action::SetRemote
            | Action::PushNotebook
    )
}

fn looks_like_git_url(s: &str) -> bool {
    s.starts_with("http://")
        || s.starts_with("https://")
        || s.starts_with("git@")
        || s.starts_with("ssh://")
        || s.starts_with("git://")
}

/// Notebook name derived from a git URL's repo name — the last path
/// segment, minus a trailing `.git`. Handles both `.../owner/repo` (split on
/// `/`) and `git@host:owner/repo.git` (split on `:` for the host separator,
/// `/` for the owner/repo one) since splitting on either character and
/// taking the last piece lands on `repo[.git]` either way.
fn notebook_name_from_git_url(url: &str) -> Option<String> {
    let trimmed = url.trim_end_matches('/');
    let last = trimmed.rsplit(['/', ':']).next()?;
    let name = last.strip_suffix(".git").unwrap_or(last);
    (!name.is_empty()).then(|| name.to_string())
}

fn shift(current: usize, delta: isize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let len = len as isize;
    let next = (current as isize + delta).rem_euclid(len);
    next as usize
}

/// The folder breadcrumb (as path components) of `note_path`'s containing
/// directory, relative to `notebook_path` — how far to descend to land on a
/// note found via search/jump instead of always assuming it's at the root.
fn relative_folder(note_path: &std::path::Path, notebook_path: &std::path::Path) -> Vec<String> {
    note_path
        .parent()
        .and_then(|dir| dir.strip_prefix(notebook_path).ok())
        .map(|rel| {
            rel.components()
                .filter_map(|c| c.as_os_str().to_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let [area] = Layout::horizontal([Constraint::Length(width.min(area.width))])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([Constraint::Length(height.min(area.height))])
        .flex(Flex::Center)
        .areas(area);
    area
}

/// The global search modal's outer popup rect — shared by rendering and by
/// mouse hit-testing so they always agree on where things are.
fn global_search_popup_area(frame_area: Rect) -> Rect {
    centered_rect(
        frame_area,
        (frame_area.width * 3 / 4).max(40),
        (frame_area.height * 2 / 3).max(10),
    )
}

/// Splits the global search popup into (input box, results list).
fn global_search_layout(popup_area: Rect) -> (Rect, Rect) {
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(popup_area);
    (chunks[0], chunks[1])
}

pub fn run<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    while !app.should_quit {
        if let Ok(size) = terminal.size() {
            app.last_frame_area = Rect::new(0, 0, size.width, size.height);
        }
        app.refresh_history_cache();
        app.refresh_folder_preview_cache();
        app.refresh_note_preview_cache();
        app.expire_status_message();
        app.poll_update_channel();
        terminal.draw(|frame| draw(frame, app))?;

        if app.want_relaunch {
            if let Some(exe_path) = app.relaunch_exe_path.take() {
                relaunch_into_updated_binary(&exe_path)?;
            }
            app.should_quit = true;
            continue;
        }

        if let Some((path, editor)) = app.want_external_edit.take() {
            let notebook_name = app.selected_notebook().map(|nb| nb.name.clone());
            suspend_and_edit(terminal, &editor, &path)?;
            app.refresh_notes_preserve_selection();
            if let Some(notebook_name) = notebook_name {
                app.note_changed(&notebook_name);
            }
        }

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    // Accept Press and Repeat, only drop Release — some
                    // terminals report held/fast-typed keys as Repeat rather
                    // than Press, and filtering to `== Press` was silently
                    // swallowing those.
                    if key.kind != KeyEventKind::Release {
                        app.on_key(key);
                    }
                }
                Event::Mouse(mouse) => app.on_mouse(mouse),
                _ => {}
            }
        }
    }
    Ok(())
}

/// Leaves the alternate screen (same teardown half as `suspend_and_edit`,
/// deliberately without the restore half — this process is exiting, not
/// resuming) and spawns the just-installed binary at the same path
/// (`install_latest` replaced it in place) so the update feels like a
/// restart rather than "go run `shiki` again yourself".
fn relaunch_into_updated_binary(exe_path: &std::path::Path) -> io::Result<()> {
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        io::stdout(),
        crossterm::event::DisableMouseCapture,
        crossterm::terminal::LeaveAlternateScreen
    )?;
    let _ = std::process::Command::new(exe_path).spawn();
    Ok(())
}

/// Leaves the alternate screen and disables raw mode so `$EDITOR` gets a
/// normal terminal, then restores everything and forces a full redraw.
fn suspend_and_edit<B: Backend>(
    terminal: &mut Terminal<B>,
    editor: &str,
    path: &std::path::Path,
) -> io::Result<()> {
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        io::stdout(),
        crossterm::event::DisableMouseCapture,
        crossterm::terminal::LeaveAlternateScreen
    )?;
    let _ = shiki_core::editor::command_for(editor, path).status();
    crossterm::execute!(
        io::stdout(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    crossterm::terminal::enable_raw_mode()?;
    terminal.clear()
}

fn draw(frame: &mut ratatui::Frame, app: &App) {
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
        app.input.render(
            frame,
            popup_area,
            kind.title(),
            hex_to_color(&app.theme.accent),
        );
    }

    if app.show_tags {
        let tags = TagIndex::build(&app.notes);
        let popup_area = centered_rect(frame.area(), 40, (tags.len() as u16 + 2).max(3));
        frame.render_widget(Clear, popup_area);
        panel_tags::render(frame, popup_area, &tags, true, &app.theme);
    }

    if app.show_theme_picker {
        render_theme_picker(frame, frame.area(), app);
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

    if app.show_history {
        render_history(frame, frame.area(), app);
    }

    if app.show_update {
        render_update(frame, frame.area(), app);
    }

    if app.show_which_key {
        which::render(frame, frame.area(), app);
    }

    // Rendered last, on top of every other modal: a confirmation (e.g. a
    // history revert) can be triggered *from inside* one of them, and must
    // stay visually on top rather than getting painted over by whichever
    // modal is still "open" underneath it.
    if let Some(dialog) = &app.confirm {
        let popup_area = centered_rect(frame.area(), (dialog.message.len() as u16 + 12).max(30), 3);
        frame.render_widget(Clear, popup_area);
        dialog.render(frame, popup_area, hex_to_color(&app.theme.warning));
    }
}

fn render_theme_picker(frame: &mut ratatui::Frame, frame_area: Rect, app: &App) {
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

fn render_global_search(frame: &mut ratatui::Frame, frame_area: Rect, app: &App) {
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
        .map(|hit| {
            let (nb, note) = &app.global_search_pool[hit.index];
            ListItem::new(format!(
                "{}  {} › {}",
                icons::NOTE,
                nb.name,
                note.frontmatter.title
            ))
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

fn render_update(frame: &mut ratatui::Frame, frame_area: Rect, app: &App) {
    let popup_area = centered_rect(frame_area, (frame_area.width * 2 / 3).max(50), 7);
    frame.render_widget(Clear, popup_area);

    let current = env!("CARGO_PKG_VERSION");
    let (title, body) = match &app.update_state {
        Some(UpdateState::Checking) => (
            " Checking for updates ".to_string(),
            "Checking GitHub Releases…".to_string(),
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
            "Downloading, verifying, and installing…".to_string(),
        ),
        Some(UpdateState::Installed(version)) => (
            " Installed ".to_string(),
            format!("Installed v{version} \u{2014} restarting shiki…"),
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

fn render_logs(frame: &mut ratatui::Frame, frame_area: Rect, app: &App) {
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
        " {}  Logs [{}]  —  y/c copy all · esc/q close ",
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

fn render_tree(frame: &mut ratatui::Frame, frame_area: Rect, app: &App) {
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
                ListItem::new(ratatui::text::Line::from(ratatui::text::Span::styled(
                    format!("{}{} {name}/", "  ".repeat(*depth), icons::NOTEBOOK),
                    Style::default().fg(muted).add_modifier(Modifier::BOLD),
                )))
            }
            crate::tree::TreeRow::Note { depth, note } => {
                ListItem::new(ratatui::text::Line::from(ratatui::text::Span::styled(
                    format!(
                        "{}{} {}",
                        "  ".repeat(*depth),
                        icons::NOTE,
                        note.frontmatter.title
                    ),
                    Style::default().fg(fg),
                )))
            }
        })
        .collect();
    let highlight_symbol = format!("{} ", icons::ARROW);
    let title = format!(
        " {}  Tree [{} notes]  —  enter open · esc/q close ",
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

fn render_history(frame: &mut ratatui::Frame, frame_area: Rect, app: &App) {
    let height = (frame_area.height * 3 / 4).max(8);
    let popup_area = centered_rect(frame_area, (frame_area.width * 3 / 4).max(50), height);
    frame.render_widget(Clear, popup_area);

    let fg = hex_to_color(&app.theme.fg);
    let muted = hex_to_color(&app.theme.muted);

    if let Some((commit_id, content)) = &app.history_viewing {
        let short = commit_id.chars().take(7).collect::<String>();
        let title = format!(
            " {}  Revision {short}  —  r revert · esc back ",
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
                ratatui::text::Span::styled(
                    format!("{} ", entry.date.format("%Y-%m-%d %H:%M")),
                    Style::default().fg(fg),
                ),
                ratatui::text::Span::styled(format!("{short}  "), Style::default().fg(muted)),
                ratatui::text::Span::styled(entry.message.clone(), Style::default().fg(fg)),
            ]))
        })
        .collect();
    let highlight_symbol = format!("{} ", icons::ARROW);
    let title = format!(
        " {}  History [{} revisions]  —  enter view · r revert · esc/q close ",
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
