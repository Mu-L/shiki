# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

**shiki** (私記) is a TUI note-taking app written in Rust, inspired by Yazi's three-pane
layout and modal navigation. Notes are plain Markdown files with YAML frontmatter,
organized into "notebooks" (directories, each its own git repo). Full design spec,
motivation, keybindings, config format, and included themes are documented in `IDEA.md` —
read it before making architectural changes, since it's the source of truth for intended
behavior (layout, CLI commands, config schema, etc).

## Commands

```sh
cargo build --workspace              # build everything
cargo check --workspace              # fast type-check (use this while iterating)
cargo clippy --workspace --all-targets   # lint; keep this clean before considering work done
cargo fmt --all                      # format (run after editing, before checking clippy)
cargo run -p shiki-cli -- <args>     # run the binary, e.g. `-- new "titulo"`, `-- daily`, no args launches the TUI
```

There is no test suite yet (`cargo test --workspace` currently runs nothing). When adding
tests, put them as `#[cfg(test)]` modules in the relevant `shiki-core`/`shiki-config` file —
those two crates are plain logic with no TUI/terminal dependency, so they're the easiest to
unit test.

To exercise the CLI without touching the real user config/data, override XDG dirs (used via
`directories::ProjectDirs::from("", "", "shiki")` in `shiki-config`):

```sh
XDG_CONFIG_HOME=/tmp/shiki-test-config XDG_DATA_HOME=/tmp/shiki-test-data \
  cargo run -p shiki-cli -- notebook create personal
```

## Versioning

`[workspace.package] version` in the root `Cargo.toml` is the single version number for the whole
app — every crate inherits it via `version.workspace = true`, so there's exactly one place to bump.
The TUI status bar shows it (right-aligned in the footer, paired with the `? help` hint) via
`env!("CARGO_PKG_VERSION")` in `shiki-tui/src/status_bar.rs`, which reads shiki-tui's own
(inherited) manifest version at compile time. Cutting a release is two steps: bump the workspace
version, add a `CHANGELOG.md` entry (follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)).

**The status bar paints no background** (`status_bar.rs` — no `.bg(...)` anywhere, spans just use
themed fg colors on the terminal's own background) and only shows what's contextually useful: the
mode label is omitted entirely in `Mode::Normal` (only INSERT/EDIT/VISUAL are worth flagging),
there's no theme-name display, and the metadata block is focus-dependent — character count of the
selected note while reading/editing one (`Focus::Notes`/`Focus::Preview` with a note selected), or
the note count in view otherwise (e.g. still browsing NOTEBOOKS). Git info comes from
`App::git_status: shiki_core::git::GitStatus` — `git::status(path, remote)` resolves `HEAD`'s
branch name and runs `repo.graph_ahead_behind` against `refs/remotes/{remote}/{branch}` (from the
last fetch — it doesn't talk to the network itself). All three counts render together when
nonzero rather than only ever showing one: `+{dirty_count}` (uncommitted files), `{UPLOAD
icon}{ahead}` (local commits not yet pushed), `{DOWNLOAD icon}{behind}` (remote commits not yet
pulled in) — a diverged branch after a `pull` can genuinely show both ahead *and* behind at once,
verified by pushing a divergent commit from a second clone and pulling: `main +0 ↑1 ↓1`. Don't
collapse this back to a single dirty/clean marker; the counts were added specifically because a
bare marker didn't say how much was pending.

**Notebook-scoped git actions (`Action::SyncNotebook`/`PullNotebook`/`PullAllNotebooks`/
`SetRemote`/`PushNotebook`) resolve regardless of which panel has focus, not only when NOTEBOOKS
does** (`handle_normal_key`'s fallback: if `resolve_scoped(self.focus, code)` misses, it also tries
`resolve_scoped(Focus::Notebooks, code)` and accepts the result only if `is_notebook_git_action`
allows it). These act on "the selected notebook" as a concept, not on "the NOTEBOOKS panel" — `u`
(push) while reading a note in PREVIEW previously did nothing at all, silently, since `u` isn't
bound in the `notes`/`preview` scope maps and the old code never fell back to `notebooks`.
Deliberately NOT extended to `NewNotebook`/`RenameNotebook`/`DeleteNotebook`: those share letters
with Notes-scope actions (`a`/`r`/`d`), so falling back to them from PREVIEW (which has no `d`
binding of its own) would silently delete the whole notebook instead of doing nothing — keep that
whitelist narrow if adding new notebook actions.

**`git::push` takes no branch argument — it pushes whatever `HEAD` actually points at.** It used to
take `branch: &str` and always push `refs/heads/{branch}:refs/heads/{branch}` using
`config.git.branch` (default `"main"`). But `pull`'s branch-fallback (above) means the local repo's
real branch can legitimately be something else, e.g. `master` — pushing the fixed configured name
then failed with "src refspec 'refs/heads/main' does not match any existing object" the moment it
didn't exist locally. `push` now resolves the branch itself via `repo.head()?.shorthand()`, so it's
never out of sync with what `pull` actually created. Don't reintroduce a `branch` parameter here;
if a caller needs to know which branch got pushed, read it off `GitStatus.branch` instead.

## Architecture

Cargo workspace, four crates with a strict one-way dependency chain:

```
shiki-core   (pure domain logic, no TUI, no config crate dependency)
shiki-config (TOML config + themes, no ratatui dependency)
shiki-tui    (ratatui UI, depends on shiki-core + shiki-config)
shiki-cli    (clap entrypoint, depends on all three; binary name is `shiki`)
```

**shiki-config is deliberately decoupled from ratatui.** `Theme` (`shiki-config/src/theme.rs`)
stores every color slot as a string — `#rrggbb` hex, a terminal-native ANSI name
(`"blue"`, `"darkgray"`, …), or `"reset"` to inherit the terminal's own default — not
`ratatui::style::Color`. Don't add a ratatui dependency to shiki-config to "simplify" this — the
string→`Color` conversion lives in `shiki-tui/src/render.rs::hex_to_color`, keeping the config
crate reusable outside a TUI context. The `"default"` built-in theme (`Theme::terminal_default`)
uses `"reset"`/ANSI names throughout specifically so it doesn't impose a fixed palette.
Included theme palettes live one-per-file under `shiki-config/src/themes/` (catppuccin, tokyo_night,
gruvbox, nord, solarized), registered in `themes/mod.rs::all()`/`by_name()`. Every palette's hex
values are taken directly from each project's own official spec (verified against
morhetz/gruvbox, tokyonight.nvim, nordtheme.com, solarized, and catppuccin — not approximated).

**`theme.selection` used to be defined by every palette but rendered nowhere.** Every `List` widget
across the TUI (Notebooks, Notes, tree, logs, global search, theme picker, which-key) set
`.highlight_style(Style::default().fg(accent).add_modifier(BOLD))` with no `.bg(...)` at all, so the
selected row only ever got bold accent-colored text — no actual highlighted background band, which
is why every theme looked "flat"/less faithful than the same palette elsewhere (editors, prompts):
none of them were using their own `selection` color for the one thing it's for. Fixed by adding
`.bg(hex_to_color(&theme.selection))` to every one of those `.highlight_style(...)` calls (verified
by inspecting live ANSI output via `tmux capture-pane -e`, not just visually — e.g. gruvbox-dark's
selected row now carries `48;2;60;56;54`, exactly `#3c3836`). `panel_tags.rs`'s `.highlight_style`
is a pre-existing, separate gap: that panel never calls `ListState::select` at all (no navigable
tags list yet), so its highlight style has never been reachable regardless of this fix — not
touched here, since adding a background there wouldn't be visible without also adding real
selection/navigation to that panel first.

**Each crate has its own error type** — there is no shared error enum:
- `shiki_core::Error`/`Result` (thiserror, in `shiki-core/src/lib.rs`)
- `shiki_config::config::Error`/`Result` (thiserror, in `shiki-config/src/config.rs`)
- `shiki-cli` uses `anyhow` at the command layer to unify both.

**Re-export asymmetry to be aware of:** `shiki_config::Config` and `shiki_config::Theme` are
re-exported at the crate root, but the nested types (`Keybindings`, `GitConfig`, `ThemeConfig`)
are not — reach them via `shiki_config::config::Keybindings` etc. `shiki_core` re-exports `Note`,
`Frontmatter`, `Notebook`, `NotebookStore`, `SearchEngine`, `TagIndex`, `Template` at the root, but
functions like `shiki_core::daily::create_or_open` and `shiki_core::git::commit_all` are only
reachable through their module path.

**Note file format** (`shiki-core/src/note.rs`): a `.md` file starting with `---\n`, YAML
frontmatter, a closing `\n---`, then the Markdown body. This is parsed/serialized manually
(`Note::split` / `Note::to_file_contents`) rather than via a frontmatter crate — keep both in
sync if the delimiter format changes.

**Frontmatter is optional on read, not just on write.** `Note::from_file` never fails on content —
`try_parse_frontmatter` only succeeds on a well-formed `---` block with valid YAML; anything else
(a plain markdown file from `nb`, an existing repo, a manual export, or even a `---` block with
broken YAML) falls through to `synthesize_frontmatter`, which derives a title from the first `#
heading` or the filename and a date from the file's mtime, treating the whole file as the body.
This is deliberate: a notebook built by pointing `git.set_remote` + pull at someone else's repo
will have files shiki never wrote, and those must still show up as notes rather than silently
vanishing (the old behavior — `list_notes` used to propagate the parse error via `?`, so a single
non-conforming file blanked out the *entire* notebook's listing). The only remaining failure mode
of `from_file` is a genuine I/O error. Nothing is rewritten on disk until the note is actually
saved (rename/edit) through shiki — reading one doesn't touch it.

**A notebook can nest folders arbitrarily deep, like `nb`** — `Notebook::list_dir(relative)`
returns `(Vec<String> folder names, Vec<Note>)` for one level; `list_notes()` is just
`list_dir(Path::new(""))` (root only, for CLI/daily-note callers that don't care about depth), and
`all_notes_recursive()` walks every level (used by `NotebookStore::all_notes` for global search).
Note CRUD takes the actual `Path` rather than reconstructing one from a root-relative slug
(`create_note_in`, `delete_note_at`, `rename_note_at`) specifically so it keeps working at any
depth — don't reintroduce slug-based root-only variants as the "normal" path. In `shiki-tui`,
`App.notes_path: Vec<String>` is the breadcrumb and `App.selected_note` indexes into the
*combined* `folders ++ notes` list, not `notes` alone — use `selected_note()`/`selected_folder()`
rather than indexing either `Vec` with it directly. `l`/`→`/`enter` on a folder descends
(`navigate_forward`); `h`/`←` ascends one folder level before falling back to `Focus::backward()`
(`navigate_backward`).

**Selecting a folder (not a note) in NOTES previews its contents in PREVIEW, not a static hint.**
`panel_preview.rs::folder_preview_lines` calls `nb.list_dir(notes_relative_path().join(folder))`
fresh on every render (a single non-recursive `read_dir`, cheap enough to redo every frame — no
cached state, same as the rest of this render function being a pure function of `App`) and lists
its subfolders then notes, or "Empty folder." if there's nothing there. This replaced a static
"press enter to open this folder" message — the point is to show what's actually inside before
descending, the same way selecting a note already shows its content. `App::notes_relative_path` is
`pub` specifically so `panel_preview.rs` (a different module) can build that sub-path.

**Collapsed (out-of-focus) panels are 1 column wide (`layout::COLLAPSED`), not 3.** At 3 columns a
collapsed panel showed a sliver of unreadable truncated content alongside its border; at 1 column
it's just the border line, which is all a collapsed panel needs to communicate — the user already
knows it's there and can `tab`/`h` back into it.

**Notebook = directory + independent git repo** (`shiki-core/src/notebook.rs`,
`shiki-core/src/git.rs`, via `git2`). `NotebookStore::create` calls `git::init_repo` immediately,
so every notebook is git-managed from creation, not lazily. Notebook names come straight from user
input (the "new notebook" prompt) and are used as a path component — `validate_name` in
`notebook.rs` rejects empty/`.`/`..`/`/`/`\` before any `create`/`rename`/`get`/`delete` joins the
name onto `root`; don't bypass it by constructing paths manually elsewhere.

**Search** uses `nucleo-matcher` (not the full `nucleo`/`nucleo-picker` crates) — see
`shiki-core/src/search.rs::SearchEngine`. `search()` matches note titles only (used by the
notebook-local jump, `/`, via `all_notes_recursive` so it still finds nested notes); `search_text()`
is the generic version over arbitrary haystacks, used by global search (leader+`g`) with
`"{notebook} {title} {body}"` per note so it matches on content too, not just titles.

**Keybindings are scoped, not one flat map** (`shiki-config/src/config.rs::Keybindings`,
`shiki-tui/src/keybindings.rs::KeyMaps`). There are four independent `HashMap<KeyCode, Action>`s —
`global` (needs the leader key first), `notebooks`, `notes`, `preview` — each populated from its
own `[keybindings.*]` TOML table. The same key can resolve to a different `Action` depending on
which map is consulted, e.g. `a` is `NewNotebook` in the `notebooks` map and `NewNote` in the
`notes` map. `App::handle_normal_key` picks the map by `self.focus` via
`KeyMaps::resolve_scoped`, except when `leader_pending` is set (one key after the leader), in
which case it resolves against `KeyMaps::resolve_global` instead — see `leader_pending` handling
at the top of `handle_normal_key`. Navigation (`hjkl`, arrows, `tab`, `enter`, `?`) and `quit` are
**not** in any scope map; they're hardcoded in `handle_normal_key` since they behave identically
everywhere (`quit` is matched via `KeyMaps::is_quit`, a plain `KeyCode` comparison, not an
`Action` variant).

**`App::on_key` dispatches on `Mode`** (`shiki-tui/src/app.rs`) — `Insert` routes to
`handle_insert_key` (drives `InputBox` for new note/notebook, rename, jump-search, set-remote, and
move-to-notebook), `Edit` routes to `handle_edit_key` (forwards keys into the `tui-textarea`-backed
`InlineEditor`, `Esc` saves and exits), `Normal`/`Visual` route to `handle_normal_key`. A delete
(note/notebook, depending on `Focus`) goes through a separate `confirm: Option<ConfirmDialog>` gate
checked before mode dispatch. External editing (`E`, or `i` when
`general.use_favorite_editor` is on) sets `want_external_edit: Option<(PathBuf, String)>` — the
editor command travels with the path since it's resolved per-invocation (static configured editor
for `E`, OS-detected favorite via `shiki_core::editor::detect_favorite_editor` for `i`) — `run()`
picks this up between draw calls to disable raw mode / leave the alternate screen, spawn the
editor via `shiki_core::editor::command_for` (splits multi-word commands like `"code --wait"`),
and restore the terminal. The theme picker (leader+`c`) live-previews by mutating `self.theme` as
you move the cursor and only persists to `config.toml` on `Enter`; `Esc` reverts to
`available_themes[theme_index]`. `shiki-tui/src/command.rs`'s `CommandPalette` is still unused
dead code — the notes-scope search (`/`) and global search (leader+`g`) were both built directly
in `App` instead.

**`PageUp`/`PageDown`/`Home`/`End` are handled explicitly everywhere they're needed — they are not
free from ratatui/tui-textarea.** The one place they *do* come for free is the inline editor
(`handle_edit_key` forwards the raw `KeyEvent` straight to `tui-textarea`'s `TextArea::input`,
which maps `Key::PageUp/PageDown` to its own `Scrolling::PageUp/PageDown` and `Key::Home/End` to
"start/end of the current line" — verified against the vendored crate source, not assumed). Every
other list/scroll in shiki-tui needed its own handling: `App::move_selection`'s `Focus::Preview`
arm used to always scroll by exactly 1 regardless of the `delta` passed in (a latent bug — nothing
called it with anything but ±1 until `PageUp`/`PageDown` needed ±`PAGE_STEP`); it now scrolls by
`delta.unsigned_abs()`. `jump_to_start`/`jump_to_end` (bound to `Home`/`End` in `handle_normal_key`)
reuse `move_selection`'s existing shift+reload logic for NOTEBOOKS/NOTES by computing the exact
delta to land on index 0 or `len - 1`, rather than duplicating that logic. The PREVIEW `End` case is
the one approximation: it can't know the note's true *rendered* (wrapped) line count from outside
`draw()`, so it clamps the raw source line count against the preview panel's actual height (via
`layout::split(self.last_frame_area, self.focus).preview.height` — reusing the exact layout
`draw()` itself uses) so it lands at the last screenful instead of overshooting into blank space
(an actual bug hit and fixed while building this — the first version scrolled straight to the raw
line count with no ceiling at all). The same four keys were also added to every scrollable modal
(logs, global search, tree view) using each one's own existing selected-index field — no new
pattern, just filling in a gap that existed since those modals were first built.

**The which-key modal (`?`, `shiki-tui/src/which.rs` + `App::open_which_key`/
`handle_which_key_key`/`which_key_filtered_entries`) is a near-full-screen searchable list, not the
old small centered `Paragraph` popup.** It used to size itself to content height clamped against
the terminal (`entries.len() + scopes + 3` capped at `area.height - 2`) with *no* scroll state at
all — anything that didn't fit was silently clipped, and `on_key` treated which-key as modal-that-
closes-on-any-key (`if self.show_which_key { self.show_which_key = false; return; }`), so it
couldn't be typed into or scrolled either. It's now a `List`/`ListState` (same widget-based pattern
as logs/tree/global-search) filling most of the frame (`which.rs::render`'s `margin_x`/`margin_y` is
`area.{width,height} / 10`), with `App.which_key_input: InputBox` filtering `KeyMaps::entries()` by
key/action-label/scope substring (case-insensitive, `App::which_key_filtered_entries`) and
`App.which_key_selected` indexing into the *filtered* list. It doubles as a command palette:
`Enter` looks up the highlighted filtered entry's `Action` and calls `handle_action` on it directly,
closing the modal first — same "search-then-execute" pattern as global search's jump-to-hit.
Typed characters go to the filter (not `j`/`k` navigation, since `j`/`k` are themselves things a
user might type to search for "jump"/"key") — navigation is arrows + PageUp/PageDown/Home/End only,
matching the convention global search already established for the same reason. Don't revert this to
`on_key`'s blanket "any key closes it," and don't add `j`/`k` as navigation shortcuts here.

**Tree view (notes-scope `T`, `shiki-tui/src/tree.rs` + `App::open_tree`/`handle_tree_key`) is a
read-only modal, not a persistent alternate mode for the Notes panel.** `tree::build(nb)` walks the
whole notebook recursively (depth-first, folders — and everything under them — before the notes at
that level, same per-level ordering the Notes panel itself uses, just applied at every depth) into
a flat `Vec<TreeRow>` computed fresh each time the modal opens; it isn't kept in sync with
`folders`/`notes` afterward; because it's just a display list. Folder rows are display-only —
`tree_selected` indexes only the `Note` rows (`App::tree_note_count`/`tree_selected_row` — the
latter maps that note-only index back to its row position for `ListState::select`, since folder
headers are interspersed). `Enter`/`l` on a note reuses the same deep-link pattern as global search
(`relative_folder` to point `notes_path` at the note's folder, `reload_notes`, select by path,
focus `Preview`) — don't reintroduce a separate persistent tree-mode toggle for the main panel;
the modal is deliberately the simpler design, consistent with how the logs/theme-picker/global-
search modals already work.

**New notebook (`a`) detects a pasted git URL and clones instead of creating a plain notebook**
(`App::confirm_input`'s `PendingInput::NewNotebook` arm → `looks_like_git_url` →
`App::create_notebook_from_url`). If the typed value starts with `http(s)://`, `git@`, `ssh://`, or
`git://`, the notebook name is derived from the URL's last path segment (minus a trailing `.git`,
via `notebook_name_from_git_url` — handles both `.../owner/repo` and `git@host:owner/repo.git`
since splitting on either `/` or `:` and taking the last piece lands on the repo name either way),
then it creates the notebook, `git::set_remote`s it, and `git::pull`s immediately — so importing an
existing repo is `a` + paste URL + Enter instead of four separate steps (new notebook, name, `R`,
pull). A plain name still takes the normal empty-input-fallback path.

**Git remote support** (`shiki-core/src/git.rs::set_remote`/`remote_url`, plus the pre-existing
`pull`/`push`/`commit_all`) lets a notebook's `origin` be a normal git URL or a local path — git2
treats both the same for fetch. `Action::PullAllNotebooks` loops every notebook and reports
`{ok} ok, {failed} failed`; notebooks with no remote configured are an expected failure there, not
a bug. `pull` handles two cases explicitly: if `refs/heads/{branch}` already exists locally it only
fast-forwards (never discards local commits); if it doesn't exist yet (a brand-new/empty notebook
being pointed at an existing remote — the "import an existing repo" flow: `a` new notebook, `R` set
remote, `p` pull) there's nothing to fast-forward against, so it creates the branch straight from
what was fetched instead, same as `git clone`'s initial checkout. Don't remove that branch: without
it, pulling into a fresh notebook fails with "reference 'refs/heads/main' not found".

**Authentication (`git.rs::build_callbacks`)** is shared by `push`/`pull` and tries, per libgit2's
`allowed: CredentialType` for that attempt: SSH agent (only if `SSH_KEY` is offered — irrelevant for
`https://` remotes), then `Cred::credential_helper` (the system's own git credential store — reuses
whatever `git`/`gh` already have cached, e.g. macOS Keychain, libsecret, Git Credential Manager),
then anonymous `Cred::default()` for public repos. An attempt counter caps retries at 5 so a server
that keeps rejecting every credential type can't loop forever. Don't go back to a bare
`Cred::ssh_key_from_agent`-only closure — that's what previously made any HTTPS remote fail with a
generic "authentication required but no callback" error.

**`pull` fetches every branch on the remote (empty refspec list, so `repo.remote()`'s default
`+refs/heads/*:refs/remotes/{remote}/*` applies) and never reads `FETCH_HEAD`.** FETCH_HEAD's
on-disk format has extra "branch '...' of '...'" annotation text after the commit id (and gains
extra lines when tags get auto-followed), which git2's loose-reference parser doesn't expect —
`repo.find_reference("FETCH_HEAD")` can fail with "corrupted loose reference file: FETCH_HEAD" even
when the fetch itself succeeded fine. Reading the commit id back via `repo.refname_to_id(&tracking_
ref)` instead sidesteps the format entirely (it's just a plain ref), and also makes `pull` immune to
an already-corrupted FETCH_HEAD left over from before this fix (verified: pre-corrupting
`.git/FETCH_HEAD` with garbage and pulling again still succeeds, since it's never read).
`opts.download_tags(AutotagOption::None)` is also set, purely to reduce noise/multi-line risk in
FETCH_HEAD for anyone still relying on it elsewhere. Don't reintroduce
`repo.find_reference("FETCH_HEAD")` as the way to learn what was fetched.

**`pull` returns the branch it actually pulled (`Result<String>`, not `Result<()>`), because it
isn't always `config.git.branch`.** After fetching, it prefers `branch` if that ref exists among
`refs/remotes/{remote}/*`; if not and there's exactly one branch on the remote, it falls back to
that one instead of failing outright — a repo whose default branch is `master` (or anything else)
shouldn't be un-pullable just because `config.git.branch` defaults to `"main"`. Ambiguous (multiple
branches, none matching `branch`) is a real error listing what's available, not a guess.
`App::pull_notebook` compares the returned branch against `config.git.branch` and reports the
mismatch in the status message so it's visible instead of silently pulling a different branch than
configured. `pull_notebook` also checks `git::remote_url(&nb.path).is_none()` *before* calling
`pull`, since `p` always acts on whichever notebook is currently selected — after switching
notebooks or relaunching that might not be the one a remote was set on, and letting it fail inside
git2 produced an opaque "remote 'origin' does not exist" with no indication of which notebook.

**Sync policy (`auto_push`/`auto_sync`/`auto_sync_every`) is resolved per notebook, not read
straight off the global `[git]` config.** `Config::sync_for(notebook_name) -> ResolvedSync`
(`shiki-config/src/config.rs`) layers an optional `[notebooks.<name>]` override
(`NotebookGitOverride`, every field `Option<_>`) on top of the global `[git]` defaults — a
notebook connected to a private work repo can have `auto_push = true` while a scratch notebook
with no remote stays untouched, without a global setting forcing one policy on every notebook.
`App::run_sync` (shared by manual `s` and the automatic trigger below) is the only thing that
should read `sync.auto_push`; don't reintroduce a direct `self.config.git.auto_push` read in
notebook-specific code.

**`App::run_sync`'s commit message names the actual files, not a bare count.**
`shiki_core::git::diff_summary(path)` buckets `repo.statuses(None)` into added/updated/renamed/
deleted, and for each bucket names the files directly when there are ≤3 (`"added (First note.md)"`),
falling back to a count only once a bucket gets big (`"12 updated"`) so the message doesn't become
unreadable; `run_sync` prefixes the joined result with `config.git.commit_prefix`. `run_sync` also
takes a `force_push: bool` and reports every step explicitly (`"committed: …"` or `"no changes to
commit"`, then `"pushed and confirmed by remote"` or the specific error) joined with `"; "` — a
terse "pushed" previously gave no way to tell whether anything had actually changed or whether the
push was real.

**`u` (`Action::PushNotebook` → `App::push_notebook`) commits (same as `s`) and always pushes,
regardless of the resolved `auto_push` policy** — it's `run_sync(nb, force_push: true)`, the
explicit "sync right now" override, not a push-only action. It used to skip the commit step
entirely, which meant repeatedly pressing `u` on a notebook with uncommitted notes kept reporting
"pushed" while the dirty count in the footer never moved, since nothing had actually been committed
— confusing, since "pushed" sounds like something happened. Manual `s` and the automatic
every-N-changes trigger (`App::note_changed`) both call `run_sync(nb, force_push: false)` instead,
so they still respect the resolved policy.

**`App::note_changed(notebook_name)` drives `auto_sync`'s every-N-changes trigger** — called after
every note create/edit (both `save_and_exit_edit` and the external-edit finalize in `run()`)/
rename/delete/move (bumps both the source and destination notebook for a move). It's a no-op
unless `Config::sync_for` says `auto_sync` is on for that notebook; when the per-notebook
`pending_changes: HashMap<String, u32>` counter (not persisted — resets to empty on relaunch)
reaches `auto_sync_every`, it calls `run_sync` and zeroes the counter. A failed push inside
`run_sync` (no internet, auth, rejected by the remote, etc.) is reported in the status/log like any
other error and never panics or blocks — the commit already succeeded locally regardless, so the
next sync attempt (manual `s`/`u`, or the next automatic trigger) just retries the push with
nothing lost.

**`git::push` actually verifies the push landed instead of trusting `Remote::push`'s `Ok(())` at
face value.** libgit2 reports the network round-trip succeeding even when the *server* rejected the
specific ref update (e.g. a rejecting pre-receive hook) — that only surfaces through the
`push_update_reference` callback, registered in `push` and turned into a real `Err` if the server
sent back a rejection status. (The far more common non-fast-forward rejection — diverged
history — is already caught earlier, directly as an `Err` from `Remote::push` itself; verified by
pushing a divergent commit from a second clone and confirming `push` reports the conflict instead
of silently claiming success.) Note: this callback path can't be exercised against a `file://`/local
path remote in tests — libgit2's local transport writes directly to the target repo's refs and
bypasses server-side hooks entirely, unlike `git push` CLI to a local path; it only matters for the
real HTTP/SSH transports pushes normally use.

**Status messages funnel through `App::set_status`, not direct `self.status_message = Some(...)`
assignment** — every call site was migrated to `set_status` specifically so each message also gets
appended to `log_history: Vec<LogEntry>` (capped at 500), since `status_message` itself only ever
holds the latest one and errors were getting silently overwritten before a user could read them.
leader+`l` opens the logs modal (`Action::ShowLogs` → `App::open_logs`/`handle_logs_key`) showing
that scrollback; `y`/`c` inside it copies the whole thing to the clipboard via
`shiki-tui/src/clipboard.rs` (OSC 52 escape sequence written straight to stdout — works through
ratatui's alternate screen in terminals that support it, e.g. kitty/iTerm2/Alacritty/WezTerm, and
over SSH/tmux with clipboard passthrough enabled — no native clipboard crate needed). If you add a
new status/error message anywhere, call `self.set_status(...)` — a raw `self.status_message =
Some(...)` assignment would skip the log history.

**`Notebook::list_notes` skips `.md` files that don't parse as a shiki note** (no `---` frontmatter)
instead of failing the whole listing — an imported/pre-existing repo commonly has plain markdown
files alongside shiki-formatted ones, and one bad file used to blank out the entire notebook's note
list with no error (the caller's `.ok().unwrap_or_default()` swallowed the failure). Skipped files
stay on disk and under git untouched; they just won't appear as notes until they have frontmatter.

**`KeyMaps` matches on `KeyCode` only, not the full `KeyEvent`.** Don't change this back to keying
on `KeyEvent`/comparing `KeyModifiers` — Shift-based bindings (`A`, `E`, `T`, `P`, `R` by default)
are configured as plain uppercase chars with no modifier syntax in `config.toml`, so matching must
stay modifier-agnostic or those bindings silently stop firing.

**Config/data locations**: resolved via `directories::ProjectDirs::from("", "", "shiki")`
(`Config::default_path`, `Config::default_data_dir`, `Config::default_templates_dir` in
`shiki-config/src/config.rs`), so they respect `$XDG_CONFIG_HOME`/`$XDG_DATA_HOME` — see the
Commands section above for testing in isolation.

`Cargo.lock` is committed intentionally — `shiki-cli` produces a binary, not a library, so the
lockfile should be checked in for reproducible builds (don't add it to `.gitignore`).
