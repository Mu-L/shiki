---
description: Cut a full shiki release end-to-end (bump, CHANGELOG, docs site, image/video scripts, push, GitHub Release, all stores, site) — the complete verified runbook.
agent: build
---

Execute the full shiki release process by loading the `deploy` skill and following its runbook
exactly (use the skill tool with name `deploy` — it lives at `.opencode/skill/deploy/SKILL.md` and
is the verified, step-by-step procedure). This command is the entry point that drives it.

Version to cut: `$1` (e.g. `0.9.2`). If omitted, derive it from the current
`[workspace.package] version` in the root `Cargo.toml` (default = next patch).

Work through the runbook **in order**, checking off each phase:

1. **Preflight** — clean tree on `main`, synced; enumerate `git log --oneline <last-tag>..main`.
2. **CHANGELOG** — audit `[Unreleased]` against those commits (run the `docs-coherence` skill first
   if unsure what's missing); rename to `[<version>] - <date>` and re-add an empty `[Unreleased]`.
3. **Version bump** — `[workspace.package]` + the three `[workspace.dependencies]` shiki-* in root
   `Cargo.toml`; `docs/index.html` JSON-LD `softwareVersion`.
4. **Docs site + config prose** — fix anything the coherence audit flagged; keep the built-in
   `/`-menu command lists in sync across `docs/documentation.html`, `IDEA.md`, and
   `shiki-config/src/config.rs` (`section_comment`).
5. **Image/video scripts** — if this release has visual headline features, add a shot to
   `scripts/screenshots.sh` and a Phase to `scripts/demo-gif.sh` (new sample notes go in
   `research/`, never `personal/`); `bash -n` both.
6. **Verify + commit + push** — `cargo check/clippy -D warnings/test/fmt` all green, then commit
   with `[PUBLISH]` in the message (`chore: cut release for vX.Y.Z [PUBLISH]`) and push to `main`.
7. **Ensure release.yml runs** — if no `Release` run appears within ~1–2 min of auto-tag finishing,
   dispatch it manually: `gh workflow run release.yml --ref main --field tag=vX.Y.Z`.
8. **Monitor release.yml** — builds → GitHub Release → crates.io → packaging manifests. If
   `update-packaging-manifests` fails at "Push Homebrew tap" (403 on `sazardev/homebrew-shiki`),
   push the formula manually from the tap clone.
9. **Skipped jobs** — if `update-screenshots`/`update-release-pages` were skipped, run
   `scripts/screenshots.sh`, `scripts/demo-gif.sh`, and
   `scripts/generate_release_pages.py --version X.Y.Z` locally, copy into `docs/`, commit, push.
10. **Verify every store** — GitHub Release assets + checksums, crates.io ×4, AUR `shiki-bin` +
    `shiki`, Homebrew tap, Scoop (byte-identical manifests), and the live site
    (`softwareVersion`, changelog page, OG card, feed). Report a store-by-store table to the user,
    noting any manual fallback that was needed.

Never `git tag`/push tags by hand (auto-tag does it). Never finish without the store-by-store
verification and a clean working tree.
