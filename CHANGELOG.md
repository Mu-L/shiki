# Changelog

All notable changes to shiki are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); this project doesn't follow strict
semver yet (pre-1.0), but version bumps are still meaningful and tracked here.

## [Unreleased]

## [0.6.0] - 2026-07-23

### Added

- Notebook drawer (`leader+b`): a collapsible left-side sidebar listing every notebook's git
  status in color (dirty count, ahead/behind), separate from the always-visible NOTEBOOKS panel —
  `j`/`k`/`Enter` or a mouse click jumps to a notebook, `n`/`i` (or clicking the minimal "New"/
  "Import" buttons at the bottom) open the same new-notebook prompt that already detects a pasted
  git URL and clones instead of creating a plain notebook.
- Per-note coloring in NOTES: each note's title is tinted by its actual git status (new → green,
  modified/renamed → the warning color, deleted → the error color) instead of only being visible
  as an aggregate count in the footer — `shiki_core::git::file_statuses`.
- A spinner (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`) in the footer while a sync/push/pull is running in the background,
  replacing the git-status segment for the duration — visible feedback that something's actually
  happening on a slow network call instead of the UI just looking idle.
- First unit tests in `shiki-tui` (`panel_drawer::tests`), covering the drawer's mouse
  hit-testing math — caught a real off-by-one in the button row's coordinates before it shipped.

### Changed

- `sync`/`push`/`pull`/`pull all` now run on a background thread (the same `std::thread` + `mpsc`
  pattern already used by the in-TUI self-updater) instead of blocking the render loop for the
  duration of the git/network call. Only one operation runs at a time; a second request while one
  is in flight is reported and dropped rather than queued. Verified live against a real local
  bare-repo remote (successful push) and an unreachable one (commit succeeds, push fails cleanly,
  state refreshes correctly) — the UI never freezes in either case.

## [0.5.1] - 2026-07-23

### Added

- `scripts/benchmark.sh`: an automated, transparent CPU/RAM/responsiveness benchmark that drives
  the real release binary headlessly via tmux across seven scenarios, from an empty notebook up to
  100,000 notes in one folder, 200 levels of nested folders, and a single 300,000-line note —
  samples real `/proc/<pid>/stat`/`status` numbers (CPU ticks, VmRSS, wall-clock time to first
  rendered frame) rather than estimating anything, so it doubles as a freeze/hang check.

### Changed

- PREVIEW's note view and folder-peek now cache their formatted output (`App::note_preview_cache`/
  `folder_preview_cache`) instead of re-formatting on every ~100ms draw tick, and borrow rather than
  clone that cached text into the `Paragraph` (`render::borrow_lines`). Previously, a selected
  note's entire body was reformatted via `markdown_to_lines` on every redraw regardless of size,
  and a selected-but-not-entered folder re-listed the directory and re-parsed every note's
  frontmatter on every redraw regardless of folder size — both scaled with content size at ~10Hz
  whether or not anything had actually changed. Verified via `scripts/benchmark.sh`'s aggressive
  scenarios: a 100,000-note folder now costs ~4.7% idle CPU (found via the same benchmark to be in
  the double digits before formatted-output caching was added) and a 300,000-line note ~9.9%, both
  with a sub-second first frame and zero measured RSS drift.

## [0.5.0] - 2026-07-23

### Added

- Notes-scope `f` creates a new (empty) subfolder at the current breadcrumb depth — previously
  folders could only be navigated, never created from the TUI; the only way to get one was to
  already have it on disk (an imported repo, or made outside shiki entirely).

## [0.4.2] - 2026-07-23

### Added

- In-TUI self-update (leader+`U`): checks GitHub Releases for a newer version without downloading
  anything, shows "Update available: vX.Y.Z → vA.B.C" if one exists, and on `enter` downloads,
  verifies (against GitHub's own per-asset sha256 digest), and installs it in place of the running
  binary — then automatically relaunches into it, no manual restart needed. Runs on a background
  thread so the TUI never freezes on the network call. Verified live end-to-end against the real
  repo: detects an available update, declines to re-flag when already current, and a full
  download → verify → install → relaunch round trip that lands on the new version's footer.

### Security

- `git2` bumped 0.19 → 0.21, closing 3 `cargo audit` "unsound" advisories
  (`Remote::list()`/`BlameHunk` signature/`Buf` dereference UB) — shiki's code never called any of
  the affected APIs, but they showed up in every audit regardless while pinned to 0.19.
  `Commit::summary()`/`Reference::shorthand()`/`Reference::name()`/`Remote::url()` all changed from
  `Option`-returning to `Result`-returning between these versions; every call site in `git.rs` was
  updated to match. `ssh`/`https` are now explicit features (git2 0.21's `default-features` became
  empty, whereas 0.19 defaulted to them) — without this, SSH remotes and `Cred::credential_helper`
  would've silently stopped working. Verified live: full push → remote commit → pull-into-fresh-
  notebook → note-history round trip against a local bare repo. `cargo audit` is down to 4
  warnings, all transitive via `syntect`/`ratatui` and not fixable from shiki's own `Cargo.toml`.
- `.github/workflows/ci.yml` now declares `permissions: contents: read` explicitly instead of
  inheriting the repo's default token permissions — it only builds/lints, never needs write access.

## [0.4.1] - 2026-07-23

### Added

- `shiki-core`/`shiki-config`/`shiki-tui`/`shiki-cli` are now published to
  [crates.io](https://crates.io/crates/shiki-cli) — `CARGO_REGISTRY_TOKEN` is configured, so
  `cargo install shiki-cli` works directly, no `--git`/`--path` needed. This is the release that
  verifies the `publish-crates` job actually publishes for real (previous tags skipped it since the
  secret wasn't set yet).

## [0.4.0] - 2026-07-22

### Added

- Automated release packaging: `.github/workflows/ci.yml` runs fmt/clippy/build on a Linux +
  Windows + macOS matrix on every push/PR; `.github/workflows/release.yml` builds release binaries
  for all four platform targets on every `v*` tag, publishes them (with checksums) as a GitHub
  Release, auto-updates the AUR (`packaging/aur/PKGBUILD`, `shiki-bin`) and
  Scoop (`packaging/scoop/shiki.json`) manifests with the new version/hashes, and (behind
  `CARGO_REGISTRY_TOKEN`/`AUR_SSH_PRIVATE_KEY` secrets, not yet configured) publishes to crates.io
  and pushes to the real AUR git repo.
- Installable via `yay`/`paru` (`shiki-bin`, once published to the AUR — requires a one-time
  manual AUR account/SSH key setup that only the repo owner can do), `scoop` (direct manifest URL,
  no bucket needed), a prebuilt binary from GitHub Releases, or `cargo install --path shiki-cli`
  from source — see the README's expanded Install section.

### Changed

- `git2` now builds with `vendored-libgit2`/`vendored-openssl`, statically linking libgit2/OpenSSL
  instead of depending on whatever (if anything) is installed on the system — required for
  reliable Windows builds (no system libgit2 there) and makes Linux/macOS builds portable too.
- Workspace path-dependencies (`shiki-core`, `shiki-config`, `shiki-tui`) now carry an explicit
  `version` alongside `path`, required for `cargo package`/`cargo publish` to succeed (previously
  failed with "dependency does not specify a version").

## [0.3.0] - 2026-07-22

### Changed

- Footer status messages now clear themselves after 2 seconds instead of sitting there until the
  next action happens to overwrite them, and are truncated to whatever footer space is actually
  left instead of overflowing. Nothing is lost either way — every message is still recorded in
  full in the logs modal (leader+`l`) regardless of how briefly or how much of it the footer shows.
- A bit more padding around the right-aligned `? help  vX.Y.Z` in the footer, so it doesn't sit
  flush against the terminal edge or the rest of the footer content.

## [0.2.0] - 2026-07-22

### Added

- Note version history (PREVIEW-scope `H`): every commit that changed the specific note being
  read, newest first — real git history, not a separate versioning system. `Enter` views a
  revision's full content, `r` reverts to it (behind a confirmation). A revert doesn't commit by
  itself; it becomes a normal pending change picked up by `s`/`u`/`auto_sync` like any other edit.
  The footer shows the count while reading a note (`{n} changes`).
- `D` (notes-scope) toggles each note's date next to its title in the NOTES list, off by default.
- The 3-panel layout is now responsive to terminal size instead of one fixed arrangement: wide
  terminals keep the original side-by-side columns; narrow-but-tall or square terminals stack the
  same panels vertically instead (still full-width, just not side-by-side); very small terminals
  show only the focused panel, full screen, with no collapsed siblings. Navigation (`hjkl`/`tab`)
  works identically at every size. Verified by resizing an actual terminal from 200×50 down to
  20×8 with no crash or broken rendering at any point.
- Footer now shows which editor mode is active — the resolved favorite editor's name (e.g. `nvim`)
  when `general.use_favorite_editor` is on, `native` (the built-in inline editor) when it's off —
  plus a new leader+`e` shortcut to toggle it on/off and persist the change immediately, instead of
  hand-editing config.toml.
- `shiki doctor`: an environment health check (config validity, data/templates dirs, `git`/`gh`
  on `$PATH`, terminal truecolor support, configured editor, notebook/remote summary). Works even
  when `config.toml` is malformed — unlike every other command, it diagnoses that instead of
  failing outright.
- `README.md` and `LICENSE` (MIT) — install (`cargo install --path shiki-cli`), update, and
  verify (`shiki --version`, `shiki doctor`) instructions for installing from a clone, since this
  isn't published to crates.io yet. Every crate's `Cargo.toml` now also carries `repository`
  (previously only set at the workspace level but never actually inherited by any crate),
  `keywords`, `categories`, and a `readme` pointing at it.
- `auto_sync`: a notebook can sync itself (commit, + push if `auto_push`) automatically every
  `auto_sync_every` note changes, instead of only on manual `s`. Push failures (no internet, auth,
  etc.) never block — the commit already happened locally, and the next attempt just retries.
- `u`: commits and always pushes, regardless of `auto_push`/`auto_sync` — the explicit "sync right
  now" override.
- Which-key (`?`) is now a near-full-screen searchable list instead of a small centered popup: type
  to filter by key/action/scope, `↑`/`↓`/`PageUp`/`PageDown`/`Home`/`End` to move the selection,
  `Enter` runs the highlighted action immediately — doubles as a fast command palette.

### Changed

- Selecting a folder (not a note) in NOTES now previews what's actually inside it in PREVIEW
  (subfolders, then notes, or "Empty folder.") instead of a static "press enter to open this
  folder" hint — same spirit as selecting a note already showing its content.
- Collapsed (out-of-focus) panels are now 1 column wide instead of 3 — just the border line,
  since that's already enough to show there's a collapsed panel there.
- Status bar footer redesigned: no background fill, no "NORMAL" mode label (only INSERT/EDIT/
  VISUAL are shown), no theme name. Shows contextual metadata instead (character count of the note
  being read, or note count while browsing notebooks), the current git branch with a dirty/needs-
  pull indicator, and groups `? help` with the version on the right.
- Notebook sync is smarter and per-notebook: commit messages are now auto-built from the diff
  (e.g. "shiki: 2 updated, 1 added") instead of a fixed generic message, so nothing needs to be
  typed by hand. `auto_push`/`auto_sync`/`auto_sync_every` can be overridden per notebook under
  `[notebooks.<name>]`, falling back to the global `[git]` defaults.
- Footer git status now shows actual counts instead of a bare marker: `+N` uncommitted files,
  `↑N` commits not yet pushed, `↓N` commits not yet pulled in — all three at once if applicable
  (e.g. a diverged branch), instead of just one dirty/clean indicator.
- The note-preview title no longer shows a `[j/k scroll]` hint (redundant once scrolling — and
  now `PageUp`/`PageDown`/`Home`/`End` — is the obvious way to move around); shows the note's date
  in a muted tone instead.

### Fixed

- Every theme's `selection` color was defined but never actually rendered anywhere — every list
  (notebooks, notes, tree, logs, global search, theme picker, which-key) only bold-colored the
  selected row's text, with no highlighted background band, making every theme look flatter/less
  faithful than it should. Selection now gets a real background highlight in each theme's own
  `selection` color.
- Notebook-level git shortcuts (`s` sync, `u` push, `p`/`P` pull, `R` set remote) previously only
  worked while the NOTEBOOKS panel had focus — pressing `u` while reading a note in PREVIEW did
  nothing at all, with no error or explanation. They now work from any panel, since they act on the
  selected notebook, not the focused panel.
- `push` failed with "src refspec 'refs/heads/main' does not match any existing object" on any
  notebook whose real branch isn't the globally configured one (e.g. `master`, via `pull`'s
  branch-fallback) — it now pushes whatever branch `HEAD` actually points at instead of a fixed
  configured name.
- `push` reported success even when a rejection only surfaced through the remote's per-ref status
  (e.g. a rejecting server-side hook) rather than as an outright transport error — now verified via
  `push_update_reference` and turned into a real, reported error.
- `u` used to push only, without committing first — repeatedly pressing it on a notebook with
  uncommitted notes reported "pushed" every time while the dirty count never moved, since nothing
  had actually been committed. `u` now commits (same as `s`) and always pushes; every step (commit
  outcome, then push outcome including confirmation) is reported explicitly instead of a terse
  "pushed".
- `PageUp`/`PageDown`/`Home`/`End` didn't work anywhere in the app — not while reading a note in
  PREVIEW, and not in the which-key popup, logs, global search, or tree view. They now work
  everywhere: a bigger jump (10 at a time) or first/last, using the same list or scroll each modal
  already navigates with `j`/`k`.
- Which-key (`?`) had no scrolling at all — content that didn't fit the small centered popup was
  silently clipped with no way to see the rest, and any keypress just closed it (so it couldn't be
  typed into either).

## [0.1.0] - 2026-07-22

Initial release.

### Added

- Three-pane TUI (Notebooks / Notes / Preview), Yazi-inspired: collapsing Miller-columns layout,
  modal navigation (`hjkl`/arrows, `tab`, leader key), no exterior padding.
- Notes are plain Markdown with YAML frontmatter; notebooks are directories, each its own git
  repo, nestable in folders to any depth (like `nb`).
- Frontmatter is optional on read: a `.md` file with no (or malformed) frontmatter — from `nb`, an
  imported repo, or a manual edit — still shows up, with title/date synthesized instead of being
  skipped.
- Config-driven keybindings, scoped by focus (`[keybindings.global/notebooks/notes/preview]` in
  `config.toml`), with a leader key for actions that aren't tied to one panel.
- Six built-in themes (catppuccin, tokyo-night, gruvbox, nord, solarized, and a terminal-native
  default that inherits the terminal's own colors) with a live-preview picker modal (leader+`c`).
- Nerd Font icon set throughout the UI.
- Per-notebook git integration: sync (commit + optional push), pull (fast-forward only, safe
  against local commits), pull-all, and set-remote — plus a fast path where pasting a git URL as
  the new-notebook name clones and imports it in one step.
- Robust git authentication (SSH agent or the system's own credential store, so it reuses
  whatever `git`/`gh` already have cached) and automatic fallback to the remote's actual default
  branch when it isn't the one configured.
- Global fuzzy search across every notebook (leader+`g`) and in-notebook fuzzy jump (`/`).
- Tags panel, daily notes, moving a note between notebooks, cycling sort order.
- Notebook tree view (`T`): every folder and note in a notebook, fully expanded in one overview,
  with jump-to-note.
- Logs modal (leader+`l`): a scrollback of every status-bar message (so errors aren't lost the
  instant the next one overwrites the status bar), with a clipboard-copy shortcut.
- Inline editor (inside the TUI) and external-editor integration (`$EDITOR`, or the OS-detected
  favorite editor).
- CLI commands alongside the TUI: `new`, `list`, `edit`, `show`, `search`, `daily`, `sync`,
  `config`, `notebook`, `theme`.
- App version shown in the status bar footer.
