# shiki (私記)

> **Personal notes, private log.**
> A TUI note-taking app in Rust — three-pane Yazi-style navigation, notebooks as
> git repos, Markdown with frontmatter, inline and external editing, themes,
> and fast fuzzy search. See [IDEA.md](IDEA.md) for the full design spec and
> [CHANGELOG.md](CHANGELOG.md) for release history.

## Install

Not published to crates.io yet — install straight from a clone:

```sh
git clone https://github.com/omar/shiki
cd shiki
cargo install --path shiki-cli
```

This builds the `shiki` binary in release mode and installs it to `~/.cargo/bin` (make sure
that's on your `$PATH` — `cargo install` will tell you if it isn't).

**Prerequisites:**
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
