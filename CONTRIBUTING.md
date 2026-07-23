# Contributing to shiki

Thanks for considering a contribution! shiki is a small project maintained in
spare time, so keeping changes focused and well-scoped makes review much
faster.

## Before you start

- For anything beyond a small fix (a new feature, a behavior change, a new
  keybinding), please open an issue first to discuss the approach. This
  avoids spending time on a PR that doesn't fit the project's direction.
- Read [IDEA.md](IDEA.md) — it's the source of truth for intended behavior
  (layout, CLI commands, config schema, keybindings) and explains the "why"
  behind a lot of design decisions.
- Check [CLAUDE.md](CLAUDE.md) for architecture notes and non-obvious
  implementation decisions across the codebase — it doubles as a developer
  guide even if you're not using an AI assistant.

## Development setup

```sh
git clone https://github.com/sazardev/shiki.git
cd shiki
cargo build --workspace
```

Useful commands while iterating:

```sh
cargo check --workspace                # fast type-check
cargo clippy --workspace --all-targets # lint — keep this clean
cargo fmt --all                        # format (run before committing)
cargo test --workspace                 # run the test suite
cargo run -p shiki-cli -- <args>       # run the binary, e.g. `-- daily`
```

To exercise the CLI or TUI without touching your real config/notes, override
the XDG dirs:

```sh
XDG_CONFIG_HOME=/tmp/shiki-test-config XDG_DATA_HOME=/tmp/shiki-test-data \
  cargo run -p shiki-cli -- notebook create test
```

## Project structure

Four crates with a strict one-way dependency chain — `shiki-core` (domain
logic) → `shiki-config` (TOML config/themes) → `shiki-tui` (ratatui UI) →
`shiki-cli` (clap entrypoint). See [CLAUDE.md](CLAUDE.md#architecture) for the
full breakdown, including why `shiki-config` deliberately has no `ratatui`
dependency.

## Tests

The project has relatively few automated tests today. When adding logic to
`shiki-core` or `shiki-config`, put unit tests in a `#[cfg(test)]` module in
the same file — those two crates have no TUI/terminal dependency, so they're
the easiest to test in isolation. For `shiki-tui`, prefer designing functions
that take plain values instead of `&App` (see `panel_drawer::drawer_hit_at`
for the pattern) so they can be unit-tested without constructing a full app.

## Before opening a pull request

- [ ] `cargo fmt --all` — formatting is enforced in CI
- [ ] `cargo clippy --workspace --all-targets` — keep it warning-free
- [ ] `cargo test --workspace` — passing
- [ ] For a user-facing change, add a short entry to the `## [Unreleased]`
      section of [CHANGELOG.md](CHANGELOG.md) (see
      [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) for the format)
- [ ] Keep the PR focused — unrelated formatting/refactor changes make review
      harder and are easier to land as a separate PR

## Commit messages and PRs

Write commit messages that explain *why*, not just *what* — the diff already
shows what changed. There's no strict conventional-commits requirement, but
clear, descriptive messages are appreciated.

Open the PR against `main`. CI (`fmt`, `clippy`, and a build matrix across
Linux/Windows/macOS) runs automatically — please make sure it's green before
requesting review.

## Reporting bugs / requesting features

Use the issue templates — they ask for the information that's actually
needed to act on a report (shiki version, OS, terminal emulator, steps to
reproduce for bugs). For security issues, see
[SECURITY.md](SECURITY.md) instead of opening a public issue.

## License

By contributing, you agree that your contributions will be licensed under the
project's [MIT License](LICENSE).
