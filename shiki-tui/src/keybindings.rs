use crossterm::event::KeyCode;
use shiki_config::config::Keybindings as KeybindingsConfig;
use std::collections::HashMap;

use crate::app::Focus;

/// Every action reachable from a keybinding, across every scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    // Global (leader-prefixed)
    ThemePicker,
    GlobalSearch,
    ToggleTags,
    ShowLogs,
    ToggleFavoriteEditor,
    // Notebooks-focus
    NewNotebook,
    RenameNotebook,
    DeleteNotebook,
    SyncNotebook,
    PullNotebook,
    PullAllNotebooks,
    SetRemote,
    PushNotebook,
    // Notes-focus
    NewNote,
    RenameNote,
    DeleteNote,
    JumpSearch,
    DailyNote,
    MoveNote,
    SortNotes,
    ToggleTreeView,
    ToggleDates,
    // Notes- and Preview-focus
    EditInline,
    EditExternal,
    // Preview-focus
    ShowHistory,
}

/// Translates a config string (e.g. `"enter"`, `"tab"`, `"a"`, `"space"`) into a `KeyCode`.
///
/// Matching is done on `KeyCode` alone, deliberately ignoring modifiers:
/// crossterm reports typed uppercase letters as `Char('A')` with the `SHIFT`
/// modifier set, so comparing full `KeyEvent`s against a hardcoded
/// `KeyModifiers::NONE` would silently drop every Shift-based binding.
fn parse_key(spec: &str) -> Option<KeyCode> {
    match spec.to_ascii_lowercase().as_str() {
        "enter" => Some(KeyCode::Enter),
        "tab" => Some(KeyCode::Tab),
        "esc" | "escape" => Some(KeyCode::Esc),
        "space" => Some(KeyCode::Char(' ')),
        "backspace" => Some(KeyCode::Backspace),
        s if s.chars().count() == 1 => Some(KeyCode::Char(spec.chars().next()?)),
        _ => None,
    }
}

fn bind(map: &mut HashMap<KeyCode, Action>, spec: &str, action: Action) {
    if let Some(code) = parse_key(spec) {
        map.insert(code, action);
    }
}

/// The full set of keymaps, one per scope. Navigation (`hjkl`, arrows, `tab`,
/// `enter`, `?`) isn't here — it's hardcoded in `app.rs` since it behaves the
/// same regardless of config. Everything else resolves against whichever
/// scope applies: `quit` is bare and universal, `global` requires the
/// leader key first, and `notebooks`/`notes`/`preview` are only consulted
/// while that panel has focus.
pub struct KeyMaps {
    leader: KeyCode,
    quit: KeyCode,
    global: HashMap<KeyCode, Action>,
    notebooks: HashMap<KeyCode, Action>,
    notes: HashMap<KeyCode, Action>,
    preview: HashMap<KeyCode, Action>,
}

impl KeyMaps {
    pub fn from_config(cfg: &KeybindingsConfig) -> Self {
        let leader = parse_key(&cfg.leader).unwrap_or(KeyCode::Char(' '));
        let quit = parse_key(&cfg.quit).unwrap_or(KeyCode::Char('q'));

        let mut global = HashMap::new();
        bind(&mut global, &cfg.global.theme_picker, Action::ThemePicker);
        bind(&mut global, &cfg.global.global_search, Action::GlobalSearch);
        bind(&mut global, &cfg.global.tags_panel, Action::ToggleTags);
        bind(&mut global, &cfg.global.logs, Action::ShowLogs);
        bind(
            &mut global,
            &cfg.global.toggle_favorite_editor,
            Action::ToggleFavoriteEditor,
        );

        let mut notebooks = HashMap::new();
        bind(&mut notebooks, &cfg.notebooks.new, Action::NewNotebook);
        bind(
            &mut notebooks,
            &cfg.notebooks.rename,
            Action::RenameNotebook,
        );
        bind(
            &mut notebooks,
            &cfg.notebooks.delete,
            Action::DeleteNotebook,
        );
        bind(&mut notebooks, &cfg.notebooks.sync, Action::SyncNotebook);
        bind(&mut notebooks, &cfg.notebooks.pull, Action::PullNotebook);
        bind(
            &mut notebooks,
            &cfg.notebooks.pull_all,
            Action::PullAllNotebooks,
        );
        bind(&mut notebooks, &cfg.notebooks.set_remote, Action::SetRemote);
        bind(&mut notebooks, &cfg.notebooks.push, Action::PushNotebook);

        let mut notes = HashMap::new();
        bind(&mut notes, &cfg.notes.new, Action::NewNote);
        bind(&mut notes, &cfg.notes.rename, Action::RenameNote);
        bind(&mut notes, &cfg.notes.delete, Action::DeleteNote);
        bind(&mut notes, &cfg.notes.edit_inline, Action::EditInline);
        bind(&mut notes, &cfg.notes.edit_external, Action::EditExternal);
        bind(&mut notes, &cfg.notes.search, Action::JumpSearch);
        bind(&mut notes, &cfg.notes.daily_note, Action::DailyNote);
        bind(&mut notes, &cfg.notes.move_to_notebook, Action::MoveNote);
        bind(&mut notes, &cfg.notes.sort, Action::SortNotes);
        bind(&mut notes, &cfg.notes.tree_view, Action::ToggleTreeView);
        bind(&mut notes, &cfg.notes.toggle_dates, Action::ToggleDates);

        let mut preview = HashMap::new();
        bind(&mut preview, &cfg.preview.edit_inline, Action::EditInline);
        bind(
            &mut preview,
            &cfg.preview.edit_external,
            Action::EditExternal,
        );
        bind(&mut preview, &cfg.preview.history, Action::ShowHistory);

        Self {
            leader,
            quit,
            global,
            notebooks,
            notes,
            preview,
        }
    }

    pub fn is_leader(&self, code: KeyCode) -> bool {
        code == self.leader
    }

    pub fn leader_key(&self) -> KeyCode {
        self.leader
    }

    pub fn is_quit(&self, code: KeyCode) -> bool {
        code == self.quit
    }

    pub fn resolve_global(&self, code: KeyCode) -> Option<Action> {
        self.global.get(&code).copied()
    }

    /// Resolves `code` against whichever scope's map matches `focus`.
    pub fn resolve_scoped(&self, focus: Focus, code: KeyCode) -> Option<Action> {
        let map = match focus {
            Focus::Notebooks => &self.notebooks,
            Focus::Notes => &self.notes,
            Focus::Preview => &self.preview,
        };
        map.get(&code).copied()
    }

    /// (scope label, key description, action) for the which-key popup,
    /// grouped by scope then sorted by key within each group.
    pub fn entries(&self) -> Vec<(&'static str, String, Action)> {
        let mut out = Vec::new();
        for (code, action) in &self.global {
            out.push(("GLOBAL (leader)", describe_key(*code), *action));
        }
        for (code, action) in &self.notebooks {
            out.push(("NOTEBOOKS", describe_key(*code), *action));
        }
        for (code, action) in &self.notes {
            out.push(("NOTES", describe_key(*code), *action));
        }
        for (code, action) in &self.preview {
            out.push(("PREVIEW", describe_key(*code), *action));
        }
        out.sort_by(|a, b| a.0.cmp(b.0).then_with(|| a.1.cmp(&b.1)));
        out
    }
}

pub fn describe_key(code: KeyCode) -> String {
    match code {
        KeyCode::Char(' ') => "space".into(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "enter".into(),
        KeyCode::Tab => "tab".into(),
        KeyCode::Esc => "esc".into(),
        KeyCode::Backspace => "backspace".into(),
        other => format!("{other:?}").to_lowercase(),
    }
}

pub fn action_label(action: Action) -> &'static str {
    match action {
        Action::ThemePicker => "pick theme",
        Action::GlobalSearch => "search all notes",
        Action::ToggleTags => "tags panel",
        Action::ShowLogs => "view logs",
        Action::ToggleFavoriteEditor => "toggle favorite editor",
        Action::NewNotebook => "new notebook",
        Action::RenameNotebook => "rename notebook",
        Action::DeleteNotebook => "delete notebook",
        Action::SyncNotebook => "git sync",
        Action::PullNotebook => "git pull",
        Action::PullAllNotebooks => "git pull (all notebooks)",
        Action::SetRemote => "set git remote",
        Action::PushNotebook => "sync + push now (ignores auto_push)",
        Action::NewNote => "new note",
        Action::RenameNote => "rename note",
        Action::DeleteNote => "delete note",
        Action::JumpSearch => "jump to note (fuzzy)",
        Action::DailyNote => "daily note",
        Action::MoveNote => "move to notebook",
        Action::SortNotes => "cycle sort order",
        Action::ToggleTreeView => "notebook tree (all notes)",
        Action::ToggleDates => "toggle note dates in list",
        Action::EditInline => "edit (insert mode)",
        Action::EditExternal => "edit externally ($EDITOR)",
        Action::ShowHistory => "note history (view/revert)",
    }
}

pub fn action_icon(action: Action) -> char {
    match action {
        Action::ThemePicker => crate::icons::EYE,
        Action::GlobalSearch | Action::JumpSearch => crate::icons::SEARCH,
        Action::ToggleTags => crate::icons::TAG,
        Action::ShowLogs => crate::icons::LIST,
        Action::ToggleFavoriteEditor => crate::icons::PENCIL,
        Action::NewNotebook | Action::NewNote => crate::icons::NOTE,
        Action::RenameNotebook | Action::RenameNote => crate::icons::PENCIL,
        Action::DeleteNotebook | Action::DeleteNote => crate::icons::WARNING,
        Action::SyncNotebook
        | Action::PullNotebook
        | Action::PullAllNotebooks
        | Action::SetRemote
        | Action::PushNotebook => crate::icons::GIT,
        Action::EditInline | Action::EditExternal => crate::icons::PENCIL,
        Action::DailyNote => crate::icons::CALENDAR,
        Action::MoveNote => crate::icons::ARROW,
        Action::SortNotes => crate::icons::COLUMNS,
        Action::ToggleTreeView => crate::icons::TREE,
        Action::ToggleDates | Action::ShowHistory => crate::icons::HISTORY,
    }
}
