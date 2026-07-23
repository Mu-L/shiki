# shiki (私記)

> **Personal notes, private log.**
> A TUI note-taking app in Rust — three-pane Yazi-style navigation, notebooks as
> independent git repos, Markdown with frontmatter, inline and external editing,
> real per-note version history, themes, and fast fuzzy search.

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![GitHub stars](https://img.shields.io/github/stars/sazardev/shiki?style=social)](https://github.com/sazardev/shiki/stargazers)
[![GitHub last commit](https://img.shields.io/github/last-commit/sazardev/shiki)](https://github.com/sazardev/shiki/commits/main)
[![GitHub repo size](https://img.shields.io/github/repo-size/sazardev/shiki)](https://github.com/sazardev/shiki)

![Rust](https://img.shields.io/badge/Rust-2021-CE422B?logo=rust&logoColor=white)
![ratatui](https://img.shields.io/badge/ratatui-0.29-blue?logo=rust&logoColor=white)
![tokio](https://img.shields.io/badge/tokio-1.x-blue?logo=rust&logoColor=white)
![git2](https://img.shields.io/badge/git2-0.19-orange?logo=git&logoColor=white)
![clap](https://img.shields.io/badge/clap-4.x-blue?logo=rust&logoColor=white)

See [IDEA.md](IDEA.md) for the full design spec (keybinding tables, config schema, theme list) and
[CHANGELOG.md](CHANGELOG.md) for release history.

## Features

- **Three-pane TUI**, Yazi-inspired: modal navigation (`hjkl`/arrows), collapsing Miller-columns
  layout, and fully **responsive to terminal size** — wide terminals get the 3-column layout,
  narrow/square ones stack panels vertically, very small ones show just the focused panel,
  full screen.
- **Notebooks are independent git repos** — plain directories under the hood, each its own repo,
  with folders nested to any depth (`nb`-style). Frontmatter is optional on read, so a plain
  Markdown file dropped in from elsewhere still shows up.
- **Real per-note version history** (not a separate versioning system): every commit that changed
  a specific note, browsable and revertible straight from the TUI.
- **Full git workflow per notebook**: manual sync/push/pull, `auto_sync` (commit + push
  automatically every N changes), per-notebook policy overrides, robust HTTPS/SSH auth that
  reuses your system's own git credential store, and automatic fallback to the remote's actual
  default branch.
- **Six built-in themes** (Catppuccin, Tokyo Night, Gruvbox, Nord, Solarized, and a terminal-native
  default that inherits your terminal's own palette) with a live-preview picker.
- **Config-driven keybindings**, scoped by focus, fully remappable in `config.toml` — nothing
  hardcoded except plain navigation.
- **Global fuzzy search** across every notebook, plus in-notebook fuzzy jump.
- **Notebook tree view**, tags panel, daily notes with templates, moving notes between notebooks,
  cycling sort order, an optional date column in the notes list.
- **Inline editor** built into the TUI, or your favorite external editor — auto-detected from
  `$VISUAL`/`$EDITOR`/the OS default, toggleable on the fly from the footer.
- **Logs modal** recording every status message (so errors don't get lost), with one-key clipboard
  copy (OSC 52) for pasting elsewhere.
- **`shiki doctor`** — an environment health check that works even with a broken config.
- **CLI commands** alongside the TUI (`new`, `list`, `edit`, `show`, `search`, `daily`, `sync`,
  `notebook`, `theme`, `config`, `doctor`) for quick one-off operations without opening the UI.

## Install

Every tagged release (`.github/workflows/release.yml`) publishes prebuilt binaries for Linux,
Windows, and macOS (Intel + Apple Silicon) as a GitHub Release, plus a `SHA256SUMS.txt`. Pick
whichever of these fits your platform:

**Arch Linux (`yay`/`paru`):**

```sh
yay -S shiki-bin      # or: paru -S shiki-bin
```

**Windows ([Scoop](https://scoop.sh)):**

```powershell
scoop install https://raw.githubusercontent.com/sazardev/shiki/main/packaging/scoop/shiki.json
```

**Prebuilt binary (Linux/Windows/macOS), no package manager:**

Download the archive for your platform from the
[latest release](https://github.com/sazardev/shiki/releases/latest), extract it, and put the
`shiki`/`shiki.exe` binary on your `$PATH`.

**With `cargo`, from [crates.io](https://crates.io/crates/shiki-cli) (any platform with a Rust
toolchain):**

```sh
cargo install shiki-cli
```

Or straight from a clone, if you want to build from a specific branch/commit instead:

```sh
git clone https://github.com/sazardev/shiki
cd shiki
cargo install --path shiki-cli
```

Either way this builds the `shiki` binary in release mode and installs it to `~/.cargo/bin` (make
sure that's on your `$PATH` — `cargo install` will tell you if it isn't). libgit2/OpenSSL are
vendored and built from source (`git2`'s `vendored-libgit2`/`vendored-openssl` features), so
there's no system libgit2/OpenSSL dependency to install separately on any platform.

**Prerequisites (source/cargo install only — prebuilt binaries/AUR/Scoop don't need a Rust
toolchain):**
- A recent stable Rust toolchain (`rustup`).
- `git` on `$PATH` — notebooks are git repos under the hood.
- A [Nerd Font](https://www.nerdfonts.com) in your terminal — the UI uses Nerd Font icons
  throughout; without one, icons render as boxes/blanks instead of glyphs.
- Optional: [`gh`](https://cli.github.com) (GitHub CLI) — if you use HTTPS remotes to private
  GitHub repos, having `gh` authenticated lets shiki's git credential lookup reuse it automatically.

Run `shiki doctor` after installing (see below) to check all of this in one shot.

## Update

```sh
cd shiki
git pull
cargo install --path shiki-cli --force
```

## Verify

```sh
shiki --version   # confirm the installed version
shiki doctor      # environment check: config, data dir, git, editor, terminal, notebooks
```

`shiki doctor` is safe to run any time, including right after install with no config yet —
it reports what's missing rather than erroring out, and works even if a config file exists but
is malformed (a normal `shiki` command would fail outright in that case; `doctor` diagnoses it).

## Quick start

```sh
shiki                       # launches the TUI — no args
shiki notebook create work  # or from the TUI: `a` while NOTEBOOKS is focused
shiki new "My first note" --notebook work
shiki daily                 # today's daily note
```

Inside the TUI, press `?` for a searchable list of every keybinding (also doubles as a command
palette — type to filter, `Enter` runs the highlighted action). Full keybinding tables, config
schema, and theme list are in [IDEA.md](IDEA.md).

## Development

See [CLAUDE.md](CLAUDE.md) for the crate layout, build/lint commands, and architecture notes.

## License

[MIT](LICENSE)
