
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use shiki_config::Config;
use shiki_core::Notebook;

use crate::app::{
    App, BatchOp, DeleteTarget, Focus, Mode, PendingInput, SelectedEntry, TrashedEntry, UpdateMsg,
    UpdateState, drawer_area, global_search_layout, global_search_popup_area,
    is_notebook_git_action, looks_like_git_url, relative_folder,
    shift, PAGE_STEP,
};
use crate::editor::InlineEditor;
use crate::icons;
use crate::input::InputBox;
use crate::keybindings::{action_label, Action};
use crate::render::{hex_to_color, panel_block};
use crate::{confirm, layout, panel_drawer, status_bar};

impl App {
    fn open_theme_picker(&mut self) {
        self.theme_picker_index = self.theme_index;
        self.show_theme_picker = true;
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
                // Only reset overrides when actually switching to a
                // different base theme — compared against `config.theme.name`
                // (the last *committed* value), not `self.theme.name` (the
                // live-preview value while browsing). Re-confirming the
                // theme that was already active with no real change used to
                // silently wipe any hand-written custom colors.
                if self.config.theme.name != self.theme.name {
                    self.config.theme.overrides = Default::default();
                }
                self.config.theme.name = self.theme.name.clone();
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
    /// Unlike `open_logs`/`open_tree` (one-directional, closed via `Esc`
    /// inside their own key handler), the drawer is a true toggle — pressing
    /// its leader binding again collapses it, matching how it was asked for
    /// ("abrir o descolapsar" with the same key).
    fn toggle_drawer(&mut self) {
        self.show_drawer = !self.show_drawer;
        if self.show_drawer {
            self.drawer_selected = self
                .selected_notebook
                .min(self.notebooks.len().saturating_sub(1));
            self.refresh_drawer_statuses();
        }
    }
    fn handle_drawer_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.show_drawer = false,
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.drawer_statuses.is_empty() {
                    self.drawer_selected = (self.drawer_selected + 1) % self.drawer_statuses.len();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if !self.drawer_statuses.is_empty() {
                    self.drawer_selected = self
                        .drawer_selected
                        .checked_sub(1)
                        .unwrap_or(self.drawer_statuses.len() - 1);
                }
            }
            KeyCode::Enter => self.jump_to_drawer_notebook(),
            // Both open the same `PendingInput::NewNotebook` prompt — it
            // already detects a pasted git URL and clones instead of
            // creating a plain notebook (`looks_like_git_url`), so "import"
            // isn't separate logic, just a second entry point into it.
            KeyCode::Char('n') | KeyCode::Char('i') => {
                self.show_drawer = false;
                self.start_input(PendingInput::NewNotebook, String::new());
            }
            _ => {}
        }
    }
    /// Jumps to whichever notebook is selected in the drawer — same
    /// `notes_path.clear()` + `reload_notes()` pair `move_selection` already
    /// uses when switching `selected_notebook` via `j`/`k` in NOTEBOOKS.
    fn jump_to_drawer_notebook(&mut self) {
        if let Some((name, _)) = self.drawer_statuses.get(self.drawer_selected) {
            if let Some(idx) = self.notebooks.iter().position(|nb| &nb.name == name) {
                self.selected_notebook = idx;
                self.notes_path.clear();
                self.reload_notes();
            }
        }
        self.show_drawer = false;
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
            // Destructive and irreversible (wipes the on-disk history too,
            // the whole point of which is surviving a crash) — behind the
            // same confirm-dialog pattern as delete note/notebook, not an
            // immediate clear on one keypress.
            KeyCode::Char('x') => {
                self.pending_clear_logs = true;
                self.confirm = Some(confirm::ConfirmDialog::new(
                    "Clear all logs? This can't be undone.",
                ));
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
    pub(crate) fn poll_update_channel(&mut self) {
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
    /// Opens the links modal for the currently selected note: its own
    /// outgoing `[[wikilinks]]` plus every other note in the notebook that
    /// links back to it. Built fresh every time it opens (like the tags
    /// modal) rather than kept in sync incrementally — cheap enough for a
    /// single note's worth of links, and it means an edit made just before
    /// opening this is never stale.
    fn open_links(&mut self) {
        let Some(note) = self.selected_note().cloned() else {
            self.set_status("select a note first".into());
            return;
        };
        let Some(nb) = self.selected_notebook() else {
            return;
        };
        let all_notes = nb.all_notes_recursive().unwrap_or_default();
        self.link_rows = crate::links_panel::build(&note, &all_notes);
        self.link_selected = 0;
        if self.link_rows.is_empty() {
            self.set_status("no links or backlinks for this note".into());
            return;
        }
        self.show_links = true;
    }
    fn handle_links_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.show_links = false,
            KeyCode::Char('j') | KeyCode::Down => {
                if self.link_selected + 1 < self.link_selectable_count() {
                    self.link_selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.link_selected = self.link_selected.saturating_sub(1);
            }
            KeyCode::PageDown => {
                self.link_selected = (self.link_selected + PAGE_STEP as usize)
                    .min(self.link_selectable_count().saturating_sub(1));
            }
            KeyCode::PageUp => {
                self.link_selected = self.link_selected.saturating_sub(PAGE_STEP as usize);
            }
            KeyCode::Home => self.link_selected = 0,
            KeyCode::End => self.link_selected = self.link_selectable_count().saturating_sub(1),
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => self.jump_to_link_selection(),
            _ => {}
        }
    }
    /// The deep link: an outgoing link jumps to its resolved note (a broken
    /// one — no matching note found — just reports that instead), a
    /// backlink always jumps since `links_panel::build` only ever includes
    /// notes that actually resolved. Same "point breadcrumb at the note's
    /// folder, reload, select, focus PREVIEW" shape as `jump_to_tree_
    /// selection`/`jump_to_tag_note`/`jump_to_global_hit`.
    fn jump_to_link_selection(&mut self) {
        let Some(row) = crate::links_panel::selected_row(&self.link_rows, self.link_selected)
        else {
            self.show_links = false;
            return;
        };
        let (note_path, title) = match &self.link_rows[row] {
            crate::links_panel::LinkRow::Outgoing {
                resolved: Some(path),
                text,
            } => (path.clone(), text.clone()),
            crate::links_panel::LinkRow::Outgoing {
                resolved: None,
                text,
            } => {
                self.set_status(format!("'{text}' doesn't match any note"));
                return;
            }
            crate::links_panel::LinkRow::Backlink { note } => {
                (note.path.clone(), note.frontmatter.title.clone())
            }
            crate::links_panel::LinkRow::Header(_) => {
                self.show_links = false;
                return;
            }
        };
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
        self.show_links = false;
    }

    /// The tags modal has two levels: the tag list itself, and (after
    /// drilling into one) the notes that carry it — reset to level 1 every
    /// time it opens, so it never reopens showing a stale drill-down from
    /// last time.
    fn toggle_tags(&mut self) {
        self.show_tags = !self.show_tags;
        if self.show_tags {
            self.tags_selected = 0;
            self.tags_viewing = None;
            self.tags_notes_selected = 0;
        }
    }
    fn handle_tags_key(&mut self, key: KeyEvent) {
        let Some(tag) = self.tags_viewing.clone() else {
            let tags = self.current_tags();
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => self.show_tags = false,
                KeyCode::Char('j') | KeyCode::Down => {
                    if self.tags_selected + 1 < tags.len() {
                        self.tags_selected += 1;
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.tags_selected = self.tags_selected.saturating_sub(1);
                }
                KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                    if let Some(tag) = tags.get(self.tags_selected) {
                        self.tags_viewing = Some(tag.clone());
                        self.tags_notes_selected = 0;
                    }
                }
                _ => {}
            }
            return;
        };

        let notes_len = self.notes_with_tag(&tag).len();
        match key.code {
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Backspace | KeyCode::Left => {
                self.tags_viewing = None;
            }
            KeyCode::Char('q') => self.show_tags = false,
            KeyCode::Char('j') | KeyCode::Down => {
                if self.tags_notes_selected + 1 < notes_len {
                    self.tags_notes_selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.tags_notes_selected = self.tags_notes_selected.saturating_sub(1);
            }
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => self.jump_to_tag_note(&tag),
            _ => {}
        }
    }
    /// The deep link from level 2 of the tags modal: every match is already
    /// in the current directory's `self.notes` (see `notes_with_tag`), so
    /// this is just "select it and close", not a full `reload_notes` jump
    /// like the tree view's/global search's cross-folder equivalents.
    fn jump_to_tag_note(&mut self, tag: &str) {
        let target = self
            .notes_with_tag(tag)
            .get(self.tags_notes_selected)
            .map(|n| n.path.clone());
        let Some(path) = target else {
            self.show_tags = false;
            return;
        };
        if let Some(idx) = self.notes.iter().position(|n| n.path == path) {
            self.selected_note = self.folders.len() + idx;
        }
        self.focus = Focus::Preview;
        self.show_tags = false;
        self.tags_viewing = None;
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
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return;
        }
        if self.show_global_search {
            if let Some(index) = self.global_search_hit_at(mouse.column, mouse.row) {
                if let Some(hit) = self.global_search_results.get(index).copied() {
                    self.jump_to_global_hit(hit.index);
                }
            }
            return;
        }
        if self.show_drawer {
            let area = drawer_area(self.last_frame_area);
            let hit = panel_drawer::drawer_hit_at(
                self.drawer_statuses.len(),
                area,
                mouse.column,
                mouse.row,
            );
            match hit {
                Some(panel_drawer::DrawerHit::Notebook(index)) => {
                    self.drawer_selected = index;
                    self.jump_to_drawer_notebook();
                }
                Some(
                    panel_drawer::DrawerHit::NewButton | panel_drawer::DrawerHit::ImportButton,
                ) => {
                    self.show_drawer = false;
                    self.start_input(PendingInput::NewNotebook, String::new());
                }
                None => {}
            }
            return;
        }
        let footer = layout::split(self.last_frame_area, self.focus).status_bar;
        if status_bar::coffee_hit_at(footer, mouse.column, mouse.row) {
            self.open_coffee_link();
        }
    }
    /// Best-effort: a browser failing to launch (no GUI, headless SSH
    /// session, etc.) shouldn't do anything worse than a status message —
    /// same "fire and forget, report the failure" spirit as external-editor
    /// spawns elsewhere in this file.
    fn open_coffee_link(&mut self) {
        match shiki_core::browser::open_url(status_bar::COFFEE_URL) {
            Ok(_) => self.set_status(format!("opening {}…", status_bar::COFFEE_URL)),
            Err(err) => self.set_status(format!("couldn't open browser: {err}")),
        }
    }
    fn start_delete_notebook(&mut self) {
        if let Some(nb) = self.selected_notebook() {
            let message = format!("Delete notebook '{}' and all its notes?", nb.name);
            self.pending_delete = Some((DeleteTarget::Notebook, nb.path.clone()));
            self.confirm = Some(confirm::ConfirmDialog::new(message));
        }
    }
    /// Handles either a note or a folder selection — folders never had a
    /// delete path at all before (`Notebook::delete_folder_at` didn't
    /// exist), so selecting one and pressing `d` used to silently no-op —
    /// and, in `Mode::Visual`, the whole selected range at once instead of
    /// just the one item under the cursor.
    fn start_delete_note(&mut self) {
        if self.mode == Mode::Visual {
            let entries = self.visual_selected_entries();
            if entries.is_empty() {
                self.set_status("nothing selected".into());
                return;
            }
            let (notes, folders) = entries.iter().fold((0, 0), |(n, f), e| match e {
                SelectedEntry::Note(_) => (n + 1, f),
                SelectedEntry::Folder(_) => (n, f + 1),
            });
            let message = format!(
                "Delete {notes} note(s) and {folders} folder(s) (and everything inside them)? Restorable with leader+u."
            );
            self.pending_batch_delete = Some(entries);
            self.confirm = Some(confirm::ConfirmDialog::new(message));
            return;
        }
        if let Some(note) = self.selected_note() {
            let message = format!(
                "Delete note '{}'? Restorable with leader+u.",
                note.file_stem()
            );
            self.pending_delete = Some((DeleteTarget::Note, note.path.clone()));
            self.confirm = Some(confirm::ConfirmDialog::new(message));
        } else if let (Some(folder), Some(nb)) = (self.selected_folder(), self.selected_notebook())
        {
            let path = nb.path.join(self.notes_relative_path()).join(folder);
            let message = format!(
                "Delete folder '{folder}' and everything inside it? Restorable with leader+u."
            );
            self.pending_delete = Some((DeleteTarget::Folder, path));
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
    /// Starts a move or copy — in `Mode::Visual`, for every item in the
    /// selected range; otherwise for whichever single note/folder is
    /// currently selected. Both branches populate the same `pending_batch`
    /// shape (a `Vec` either way, just one entry long in the single-item
    /// case), so `apply_pending_batch` has exactly one code path regardless
    /// of how many things are being acted on.
    fn start_move_or_copy(&mut self, op: BatchOp) {
        if self.selected_notebook().is_none() {
            self.set_status("no notebook selected".into());
            return;
        }
        let verb = if op == BatchOp::Copy { "Copy" } else { "Move" };
        let (entries, label) = if self.mode == Mode::Visual {
            let entries = self.visual_selected_entries();
            if entries.is_empty() {
                self.set_status("nothing selected".into());
                return;
            }
            let label = format!("{} items", entries.len());
            (entries, label)
        } else if let Some(note) = self.selected_note() {
            (
                vec![SelectedEntry::Note(note.path.clone())],
                format!("'{}'", note.frontmatter.title),
            )
        } else if let Some(folder) = self.selected_folder() {
            let Some(nb) = self.selected_notebook() else {
                self.set_status("no notebook selected".into());
                return;
            };
            let path = nb.path.join(self.notes_relative_path()).join(folder);
            (vec![SelectedEntry::Folder(path)], format!("'{folder}'"))
        } else {
            self.set_status("nothing selected".into());
            return;
        };
        self.pending_input_title = Some(format!(" {verb} {label} to "));
        self.pending_batch = Some((op, entries));
        let prefill = self.current_address();
        self.start_input(PendingInput::MoveOrCopy, prefill);
    }
    /// Applies whichever `pending_batch` is waiting (move or copy, one item
    /// or many) to the parsed target — the single code path for both the
    /// single-selection case (`start_move_or_copy`) and `Mode::Visual`'s
    /// batch case, since both populate the exact same shape.
    fn apply_pending_batch(&mut self, target: &str) {
        let Some((op, entries)) = self.pending_batch.take() else {
            return;
        };
        let Some(source_nb) = self.selected_notebook().cloned() else {
            return;
        };
        let (dest_notebook, dest_relative) = match self.parse_move_target(target) {
            Ok(v) => v,
            Err(e) => {
                self.set_status(e);
                return;
            }
        };
        let verb = if op == BatchOp::Copy {
            "copied"
        } else {
            "moved"
        };
        let (mut ok, mut failed) = (0u32, 0u32);
        let mut first_err = None;
        for entry in &entries {
            let result = match (entry, op) {
                (SelectedEntry::Note(path), BatchOp::Move) => source_nb
                    .move_note_to(path, &dest_notebook, &dest_relative)
                    .map(|_| ()),
                (SelectedEntry::Note(path), BatchOp::Copy) => source_nb
                    .copy_note_to(path, &dest_notebook, &dest_relative)
                    .map(|_| ()),
                (SelectedEntry::Folder(path), BatchOp::Move) => path
                    .strip_prefix(&source_nb.path)
                    .map_err(|_| shiki_core::Error::NoteNotFound(path.display().to_string()))
                    .and_then(|relative| {
                        source_nb.move_folder_to(relative, &dest_notebook, &dest_relative)
                    }),
                (SelectedEntry::Folder(path), BatchOp::Copy) => path
                    .strip_prefix(&source_nb.path)
                    .map_err(|_| shiki_core::Error::NoteNotFound(path.display().to_string()))
                    .and_then(|relative| {
                        source_nb.copy_folder_to(relative, &dest_notebook, &dest_relative)
                    }),
            };
            match result {
                Ok(()) => ok += 1,
                Err(e) => {
                    failed += 1;
                    first_err.get_or_insert(e);
                }
            }
        }
        self.reload_notes();
        self.note_changed(&source_nb.name);
        self.note_changed(&dest_notebook.name);
        let count = entries.len();
        if failed == 0 {
            self.set_status(format!(
                "{verb} {count} item{} to '{}'",
                if count == 1 { "" } else { "s" },
                target
            ));
        } else {
            let suffix = first_err.map_or(String::new(), |e| format!(" ({e})"));
            self.set_status(format!(
                "{verb} {ok}/{count} to '{target}', {failed} failed{suffix}"
            ));
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
    /// Opens the template picker for a note titled `title` — every `.md`
    /// file in the templates dir, plus a leading "blank" option, listed
    /// fresh each time (mirrors the tags/tree modals' "rebuild on open"
    /// approach) so a template added or removed between two `a` presses is
    /// always reflected without needing its own invalidation logic.
    fn open_template_picker(&mut self, title: String) {
        self.pending_new_note_title = title;
        let mut options = vec![None];
        if let Ok(dir) = Config::default_templates_dir() {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                let mut names: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .filter_map(|e| {
                        let path = e.path();
                        if path.extension().and_then(|s| s.to_str()) != Some("md") {
                            return None;
                        }
                        path.file_stem().map(|s| s.to_string_lossy().to_string())
                    })
                    .collect();
                names.sort();
                options.extend(names.into_iter().map(Some));
            }
        }
        self.template_picker_options = options;
        self.template_picker_index = 0;
        self.show_template_picker = true;
    }
    fn handle_template_picker_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.show_template_picker = false;
                self.pending_new_note_title.clear();
                self.set_status("new note cancelled".into());
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.template_picker_index + 1 < self.template_picker_options.len() {
                    self.template_picker_index += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.template_picker_index = self.template_picker_index.saturating_sub(1);
            }
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                self.confirm_template_choice();
            }
            _ => {}
        }
    }
    /// Creates the note with the chosen template's rendered body (or an
    /// empty one, for "blank") — the actual creation `confirm_input`'s old
    /// `NewNote` arm used to do directly, now happening here once a
    /// template's actually been picked instead of always being empty.
    fn confirm_template_choice(&mut self) {
        let title = std::mem::take(&mut self.pending_new_note_title);
        let template_choice = self
            .template_picker_options
            .get(self.template_picker_index)
            .cloned()
            .flatten();
        self.show_template_picker = false;

        let body = match &template_choice {
            Some(name) => Config::default_templates_dir()
                .ok()
                .and_then(|dir| shiki_core::Template::load(&dir, name).ok())
                .map(|template| {
                    let mut vars = std::collections::HashMap::new();
                    vars.insert("title", title.clone());
                    vars.insert("date", chrono::Local::now().format("%Y-%m-%d").to_string());
                    template.render(&vars)
                })
                .unwrap_or_default(),
            None => String::new(),
        };

        match self.selected_notebook().cloned() {
            Some(nb) => match nb.create_note_in(&self.notes_relative_path(), &title, body) {
                Ok(mut note) => {
                    if let Some(name) = &template_choice {
                        note.frontmatter.template = Some(name.clone());
                        let _ = note.save();
                    }
                    self.reload_notes();
                    self.focus = Focus::Notes;
                    if let Some(idx) = self.notes.iter().position(|n| n.path == note.path) {
                        self.selected_note = self.folders.len() + idx;
                    }
                    self.set_status(format!("created '{title}'"));
                    // Drop straight into the inline editor — a fresh note
                    // (blank or templated) isn't useful to just sit on.
                    self.start_edit_inline();
                }
                Err(e) => self.set_status(format!("could not create note: {e}")),
            },
            None => self.set_status("create a notebook first".into()),
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
            Action::ToggleTags => self.toggle_tags(),
            Action::ShowLogs => self.open_logs(),
            Action::CheckForUpdate => self.open_update_check(),
            Action::ToggleDrawer => self.toggle_drawer(),
            Action::UndoDelete => self.undo_delete(),

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
            Action::MoveNote => self.start_move_or_copy(BatchOp::Move),
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
            Action::ShowLinks => self.open_links(),
            Action::ToggleFavoriteEditor => self.toggle_favorite_editor(),
            Action::ToggleVisual => self.toggle_visual(),
            Action::CopyEntries => {
                if self.mode == Mode::Visual {
                    self.start_move_or_copy(BatchOp::Copy);
                } else {
                    self.set_status("select items first with v".into());
                }
            }

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
                // The note itself isn't created yet — `open_template_picker`
                // takes over from here and creates it once a template (or
                // "blank") is actually chosen.
                self.open_template_picker(title);
                self.mode = Mode::Normal;
                return;
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
                        Ok(nb) => {
                            self.reload_notebooks();
                            if let Some(idx) = self.notebooks.iter().position(|n| n.name == name) {
                                self.selected_notebook = idx;
                                self.reload_notes();
                            }
                            let mut status = format!("notebook '{name}' created");
                            // Auto-configure a remote from `git.remote_template`
                            // (e.g. "git@git.example.com:notes/{notebook}.git")
                            // — the remote still has to already exist on that
                            // server; this doesn't create one via any hosting
                            // API. Not a push yet: a fresh notebook has no
                            // commits, so there's nothing to push until the
                            // first note is created/synced — the existing
                            // auto_push/auto_sync machinery picks it up from
                            // here naturally.
                            if !self.config.git.remote_template.is_empty() {
                                let url =
                                    self.config.git.remote_template.replace("{notebook}", &name);
                                match shiki_core::git::set_remote(&nb.path, &url) {
                                    Ok(()) => {
                                        let redacted = shiki_core::git::redact_credentials(&url);
                                        status = format!("{status}, remote set to '{redacted}'");
                                    }
                                    Err(e) => {
                                        status = format!("{status}, but could not set remote: {e}")
                                    }
                                }
                            }
                            self.set_status(status);
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
                        Ok(()) => {
                            let redacted = shiki_core::git::redact_credentials(&value);
                            self.set_status(format!("remote set to '{redacted}'"));
                        }
                        Err(e) => self.set_status(format!("could not set remote: {e}")),
                    }
                }
            }
            Some(PendingInput::MoveOrCopy) => {
                if value.is_empty() {
                    self.set_status("move/copy cancelled (empty)".into());
                    self.pending_batch = None;
                } else {
                    self.apply_pending_batch(&value);
                }
            }
            None => {}
        }
        self.pending_input_title = None;
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
                            let mut trashed = false;
                            if let Some(nb) = self.selected_notebook().cloned() {
                                let suffix = chrono::Local::now().timestamp_millis().to_string();
                                match self.trash_path(&nb, &path, &suffix) {
                                    Some(entry) => {
                                        self.last_trash = Some(vec![entry]);
                                        trashed = true;
                                    }
                                    None => {
                                        let _ = nb.delete_note_at(&path);
                                        self.last_trash = None;
                                    }
                                }
                                self.note_changed(&nb.name);
                            }
                            self.reload_notes();
                            self.set_status(if trashed {
                                format!("deleted '{name}' (undo: leader+u)")
                            } else {
                                format!("deleted '{name}'")
                            });
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
                        DeleteTarget::Folder => {
                            let name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            let mut trashed = false;
                            if let Some(nb) = self.selected_notebook().cloned() {
                                let suffix = chrono::Local::now().timestamp_millis().to_string();
                                match self.trash_path(&nb, &path, &suffix) {
                                    Some(entry) => {
                                        self.last_trash = Some(vec![entry]);
                                        trashed = true;
                                    }
                                    None => {
                                        if let Ok(relative) = path.strip_prefix(&nb.path) {
                                            let _ = nb.delete_folder_at(relative);
                                        }
                                        self.last_trash = None;
                                    }
                                }
                                self.note_changed(&nb.name);
                            }
                            self.reload_notes();
                            self.set_status(if trashed {
                                format!("deleted folder '{name}' (undo: leader+u)")
                            } else {
                                format!("deleted folder '{name}'")
                            });
                        }
                    }
                } else if let Some((note_path, commit_id)) = self.pending_revert.take() {
                    self.perform_revert(&note_path, &commit_id);
                } else if self.pending_clear_logs {
                    self.pending_clear_logs = false;
                    self.clear_logs();
                } else if let Some(entries) = self.pending_batch_delete.take() {
                    self.apply_batch_delete(entries);
                }
            }
            _ => {
                self.pending_delete = None;
                self.pending_revert = None;
                self.pending_clear_logs = false;
                self.pending_batch_delete = None;
            }
        }
        self.confirm = None;
    }
    /// `Mode::Visual`'s `d`, once confirmed — deletes every captured entry
    /// (best-effort: one failure doesn't stop the rest) and always exits
    /// back to `Mode::Normal` afterward, since the selected range no longer
    /// means anything once its contents are gone.
    fn apply_batch_delete(&mut self, entries: Vec<SelectedEntry>) {
        let Some(nb) = self.selected_notebook().cloned() else {
            return;
        };
        // Shared across the whole batch, with a per-entry index appended
        // (`trash_path`'s `suffix`) — same-named items from different
        // folders deleted in the same batch still can't collide in the
        // trash, the same reasoning as a single delete's own timestamp.
        let batch_id = chrono::Local::now().timestamp_millis();
        let (mut ok, mut failed) = (0u32, 0u32);
        let mut trashed = Vec::new();
        for (index, entry) in entries.iter().enumerate() {
            let path: &std::path::Path = match entry {
                SelectedEntry::Note(path) | SelectedEntry::Folder(path) => path,
            };
            let suffix = format!("{batch_id}-{index}");
            if let Some(entry) = self.trash_path(&nb, path, &suffix) {
                trashed.push(entry);
                ok += 1;
                continue;
            }
            let result = match entry {
                SelectedEntry::Note(path) => nb.delete_note_at(path),
                SelectedEntry::Folder(path) => path
                    .strip_prefix(&nb.path)
                    .map_err(|_| shiki_core::Error::NoteNotFound(path.display().to_string()))
                    .and_then(|relative| nb.delete_folder_at(relative)),
            };
            match result {
                Ok(()) => ok += 1,
                Err(_) => failed += 1,
            }
        }
        let any_trashed = !trashed.is_empty();
        self.last_trash = any_trashed.then_some(trashed);
        self.note_changed(&nb.name);
        self.reload_notes();
        self.mode = Mode::Normal;
        if failed == 0 {
            self.set_status(if any_trashed {
                format!("deleted {ok} item(s) (undo: leader+u)")
            } else {
                format!("deleted {ok} item(s)")
            });
        } else {
            self.set_status(format!("deleted {ok} item(s), {failed} failed"));
        }
    }
    /// Moves `path` (an absolute path to a note file or a whole folder)
    /// into the trash for notebook `nb`, tagged with `suffix` (unique per
    /// call — see the batch-delete call site for why). `None` if the trash
    /// directory couldn't be resolved or the move itself failed, in which
    /// case the caller should fall back to actually deleting `path`
    /// outright — a delete the user just confirmed should always visibly
    /// remove the item; trash is a safety net on top of that, not a
    /// precondition for it.
    fn trash_path(
        &self,
        nb: &Notebook,
        path: &std::path::Path,
        suffix: &str,
    ) -> Option<TrashedEntry> {
        let root = self.trash_root.as_ref()?;
        let trash_dir = root.join(&nb.name);
        let trash_path = shiki_core::trash::move_to_trash(path, &trash_dir, suffix).ok()?;
        Some(TrashedEntry {
            notebook: nb.name.clone(),
            original_path: path.to_path_buf(),
            trash_path,
        })
    }
    /// Restores everything from the last delete back to where it came from
    /// (leader+`u`) — a no-op, reported as such, if nothing's been deleted
    /// yet this session or a later delete already replaced the undo slot.
    fn undo_delete(&mut self) {
        let Some(entries) = self.last_trash.take() else {
            self.set_status("nothing to undo".into());
            return;
        };
        let (mut ok, mut failed) = (0u32, 0u32);
        let mut notebooks_touched = std::collections::HashSet::new();
        for entry in &entries {
            match shiki_core::trash::restore(&entry.trash_path, &entry.original_path) {
                Ok(()) => {
                    ok += 1;
                    notebooks_touched.insert(entry.notebook.clone());
                }
                Err(_) => failed += 1,
            }
        }
        for name in &notebooks_touched {
            self.note_changed(name);
        }
        self.reload_notes();
        if failed == 0 {
            self.set_status(format!("restored {ok} item(s)"));
        } else {
            self.set_status(format!("restored {ok} item(s), {failed} failed"));
        }
    }
    fn handle_insert_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.pending_input = None;
                self.pending_input_title = None;
                self.pending_batch = None;
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
            KeyCode::Esc if self.mode == Mode::Visual => self.mode = Mode::Normal,
            // Yazi-style: right/l/enter opens (a folder, or one level deeper
            // panel-wise), left/h backs out (up a folder, then a panel) —
            // suspended while `Mode::Visual` is selecting: entering/leaving
            // a folder reloads the underlying list and would strand
            // `visual_anchor` pointing at a completely different one.
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter if self.mode != Mode::Visual => {
                self.navigate_forward()
            }
            KeyCode::Char('h') | KeyCode::Left if self.mode != Mode::Visual => {
                self.navigate_backward()
            }
            KeyCode::Tab if self.mode != Mode::Visual => self.focus = self.focus.next(),
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
            self.handle_tags_key(key);
            return;
        }
        if self.show_theme_picker {
            self.handle_theme_picker_key(key);
            return;
        }
        if self.show_template_picker {
            self.handle_template_picker_key(key);
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
        if self.show_links {
            self.handle_links_key(key);
            return;
        }
        if self.show_history {
            self.handle_history_key(key);
            return;
        }
        if self.show_drawer {
            self.handle_drawer_key(key);
            return;
        }
        match self.mode {
            Mode::Insert => self.handle_insert_key(key),
            Mode::Edit => self.handle_edit_key(key),
            Mode::Normal | Mode::Visual => self.handle_normal_key(key),
        }
    }
}
