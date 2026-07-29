#!/usr/bin/env bash
# Generates marketing screenshots of the shiki TUI: every built-in theme, at
# every responsive layout tier (columned/stacked/single), covering the main
# screens and modals — real terminal rendering (xterm under Xvfb), not a
# mockup.
#
# Usage: scripts/screenshots.sh [output-dir]
#   Defaults to screenshots/ at the repo root. Wipes and regenerates it.
#
# Requires (local dev machine only — this is not part of any CI/release
# pipeline, deliberately): xterm, imagemagick (for import/identify), xdotool,
# Xvfb, and a Nerd Font (for the UI's icons — this script reuses whichever
# one is already installed via `fc-list`, falling back to plain "monospace"
# if none is found, which will render icons as boxes).
#
#   Arch/CachyOS:   sudo pacman -S xterm imagemagick xdotool xorg-server-xvfb
#   Debian/Ubuntu:  sudo apt install xterm imagemagick xdotool xvfb
#
# Runs its own virtual display (Xvfb) rather than reusing whatever $DISPLAY
# is already set — real desktop/WSLg compositors can leave window content
# unreadable via XGetImage (hit this exact issue building the script:
# BadMatch on X_GetImage against a WSLg-hosted window, immune under a plain
# Xvfb framebuffer since there's no RDP-forwarding layer in the way) — so
# this doesn't depend on, or interfere with, any real display you have.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${1:-$ROOT/screenshots}"
WORK="$(mktemp -d)"
XVFB_DISPLAY=":97"

cleanup() {
  pkill -9 -f "Xvfb $XVFB_DISPLAY" 2>/dev/null || true
  pkill -9 -f "shiki-ss-" 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT

for tool in xterm import xdotool Xvfb; do
  command -v "$tool" >/dev/null || {
    echo "error: $tool not found on \$PATH — see the header of this script for what's needed." >&2
    exit 1
  }
done

# A Nerd Font is required for the UI's icons to render as glyphs instead of
# boxes/mojibake — reuse whatever's already installed rather than assuming
# a specific one. `fc-list` is captured into a variable *before* grep ever
# runs, rather than piped straight into it — a previous version of this
# line (`fc-list | grep -im1 ... | head -1`) sent `head` closing the pipe
# early a SIGPIPE under `set -o pipefail`, which was "fixed" by switching to
# `grep -m1` (it stops reading after its own first match instead of relying
# on `head` to do it) — except `grep -m1` closing the pipe early is exactly
# as capable of SIGPIPE'ing `fc-list` itself once there are enough matching
# lines, which is a real, reproducible failure on a machine with a large
# installed Nerd Font family (confirmed: `fc-list | grep -im1 ...` alone
# exits 141 here). Capturing to a variable first means `fc-list` always
# runs to completion on its own terms, with nothing downstream that can
# close its pipe early — the actual fix, not just a different flavor of
# the same race.
FC_LIST_OUTPUT="$(fc-list)"
NERD_FONT="$(grep -im1 "nerd font mono" <<< "$FC_LIST_OUTPUT" | cut -d: -f2 | sed 's/^ *//' | cut -d, -f1)"
NERD_FONT="${NERD_FONT:-monospace}"

BIN="$ROOT/target/release/shiki"
# Always run through cargo rather than just checking `-x "$BIN"` — a stale
# release binary from an earlier version would otherwise be reused as-is
# (cargo's own up-to-date check makes this a fast no-op when nothing changed).
echo "Building release binary..."
cargo build --release -p shiki-cli --manifest-path "$ROOT/Cargo.toml"

echo "Starting Xvfb on $XVFB_DISPLAY..."
Xvfb "$XVFB_DISPLAY" -screen 0 1920x1080x24 >"$WORK/xvfb.log" 2>&1 &
sleep 1
export DISPLAY="$XVFB_DISPLAY"

THEMES=(
  catppuccin-mocha catppuccin-macchiato catppuccin-frappe catppuccin-latte
  tokyo-night-storm tokyo-night tokyo-night-moon
  gruvbox-dark gruvbox-light
  nord
  solarized-dark solarized-light
  dracula one-dark monokai
  default
)

# xterm reserves a small `internalBorder` margin around the text grid that
# the running program never draws into — it's filled with xterm's own `-bg`
# resource, which defaults to white. Without setting `-bg` explicitly, every
# capture showed a thin white border around an otherwise fully-themed
# screenshot, no matter what theme shiki itself was rendering. Extracted
# straight from each theme's own Rust source rather than hardcoded here
# twice, so it can't drift if a theme's palette changes. "default" (the
# terminal-inheriting theme) has no fixed hex — falls back to plain black,
# a reasonable generic terminal background for that one capture.
theme_bg() {
  local theme="$1" hex
  hex="$(awk -v name="\"$theme\"" '
    $0 ~ ("name: " name) { found=1; next }
    found { if (match($0, /#[0-9a-fA-F]{6}/)) print substr($0, RSTART, RLENGTH); exit }
  ' "$ROOT"/shiki-config/src/themes/*.rs 2>/dev/null)"
  echo "${hex:-#000000}"
}

rm -rf "$OUT"
mkdir -p "$OUT"

# --- Sample data: a couple of notebooks with real-looking notes, committed
# so the footer shows a clean synced state instead of a distracting dirty
# marker. Shared across every theme/size run below (the app doesn't mutate
# it materially aside from one daily-note capture near the end of the full
# depth pass, which is fine to repeat/overwrite per theme).
DATA="$WORK/data/shiki"
mkdir -p "$DATA/personal" "$DATA/work"

write_note() {
  local nb="$1" file="$2" title="$3" date="$4" tags="$5" body="$6"
  cat >"$DATA/$nb/$file" <<EOF
---
title: $title
date: $date
tags: $tags
notebook: $nb
links: []
template: null
---

$body
EOF
}

setup_sample_data() {
  for nb in personal work; do
    git -C "$DATA/$nb" init -q
    git -C "$DATA/$nb" config user.email "demo@shiki.dev"
    git -C "$DATA/$nb" config user.name "shiki demo"
  done

  write_note personal "recipe-homemade-pasta.md" "Recipe: homemade pasta" "2026-07-18" "[cooking, recipes]" \
"## Ingredients

- 400g 00 flour
- 4 large eggs
- pinch of salt
- olive oil

## Method

1. Mound the flour on a clean surface, make a well in the center.
2. Crack the eggs into the well, add salt and a splash of olive oil.
3. Whisk the eggs gradually incorporating flour from the inner rim.
4. Knead for 10 minutes until smooth and elastic.
5. Rest wrapped in plastic for 30 minutes before rolling.

Best paired with a simple brown butter and sage sauce."

  write_note personal "book-recommendations.md" "Book recommendations" "2026-07-15" "[reading, books]" \
"## Currently reading

- *The Pragmatic Programmer* — Hunt & Thomas
- *Project Hail Mary* — Andy Weir

## Queue

- *A Philosophy of Software Design* — John Ousterhout
- *The Left Hand of Darkness* — Ursula K. Le Guin

Recommended by a friend: anything by Ted Chiang, starting with *Exhalation*."

  write_note personal "weekend-hiking-trip.md" "Weekend hiking trip" "2026-07-10" "[outdoors, planning]" \
"Planning a two-day trip along the ridge trail.

- Trailhead parking fills up early, arrive before 7am
- Pack layers, weather can flip fast above 2000m
- Water refill point at the halfway hut
- Reserve the shelter two weeks in advance"

  write_note work "meeting-notes-q3-planning.md" "Meeting notes: Q3 planning" "2026-07-20" "[work, planning]" \
"## Attendees

Product, Engineering, Design leads.

## Decisions

- Ship the notebook tree view before the Q3 review
- Push the mobile companion app to Q4
- Design to finalize the onboarding flow by end of month

## Action items

- [ ] Circulate the roadmap doc for async feedback
- [ ] Schedule design review for the onboarding flow
- [ ] Follow up with infra team on the migration timeline"

  write_note work "onboarding-checklist.md" "Onboarding checklist" "2026-07-12" "[work, hr]" \
"## First week

1. Laptop + accounts provisioned
2. Repo access granted, clone the monorepo
3. Pair with a buddy on a starter ticket
4. Read the architecture overview doc

## First month

- Ship a small, well-scoped fix end to end
- Meet 1:1 with your manager and skip-level
- Present at a team demo"

  # Deliberately empty body — the /-menu screenshot (below) needs a note
  # with nothing in it, so the very first keystroke lands at column 0 of an
  # empty line and reliably triggers the menu, instead of scripting cursor
  # movement into the middle of a real note's content.
  write_note personal "quick-capture.md" "Quick capture" "2026-07-20" "[]" ""

  # Three lines all starting with the exact same word, deliberately — the
  # multi-cursor screenshots (below) drive this with repeated Ctrl+D
  # presses (select word under cursor, then keep adding the next
  # occurrence), which only needs the cursor to start at (0, 0) — the
  # default position on entering edit mode — and never needs pixel-precise
  # mouse coordinates or counted arrow-key navigation the way an Alt+Click
  # or Ctrl+Alt+Down demo would.
  write_note personal "errands-this-week.md" "Errands this week" "2026-07-21" "[personal, todo]" \
"TODO: buy milk
TODO: call the dentist
TODO: finish the quarterly report"

  for nb in personal work; do
    git -C "$DATA/$nb" add -A
    git -C "$DATA/$nb" commit -q -m "shiki: initial notes"
  done
}

setup_sample_data

CONFIG_BASE="$WORK/config-base/shiki"
mkdir -p "$CONFIG_BASE"
XDG_CONFIG_HOME="$WORK/config-base" XDG_DATA_HOME="$DATA/.." timeout 2 "$BIN" config >/dev/null 2>&1 || true

# `line_numbers`/`multi_cursor` default off (see `[editor]` in
# shiki-config) — flipped on here, once, in the base config every theme's
# own copy is `sed`'d from below, so the editor screenshots show these
# features active rather than screenshotting the (invisible-by-design)
# off state. `mouse_selection`/`find_replace` already default to `true`,
# so nothing to flip for those.
sed -i 's/^line_numbers = false/line_numbers = true/' "$CONFIG_BASE/config.toml"
sed -i 's/^multi_cursor = false/multi_cursor = true/' "$CONFIG_BASE/config.toml"

# --- One xterm session per (theme, size). xdotool sends keys/text directly
# to the window by id (a plain windowfocus first, since Xvfb has no window
# manager to mediate `_NET_ACTIVE_WINDOW`-based activation).
capture() {
  local theme="$1" size_dir="$2" cols="$3" rows="$4"
  local cfg_dir="$WORK/config-$theme"
  mkdir -p "$cfg_dir/shiki"
  sed "s/^name = .*/name = \"$theme\"/" "$CONFIG_BASE/config.toml" >"$cfg_dir/shiki/config.toml"

  # Unique per capture, not a shared literal — a leftover window from a
  # previous capture (still closing down) matching a shared title could
  # otherwise be picked up instead of this run's actual window.
  local title="shiki-ss-$theme-$size_dir"
  local bg
  bg="$(theme_bg "$theme")"
  # `-xrm` zeroes out xterm's own internal/outer border entirely (rather
  # than just recoloring it to match) — the cleanest fix, since it removes
  # the artifact instead of masking it; `-bg` is still set as a safety net
  # for any edge pixel the border removal doesn't reach.
  LANG=C.utf8 LC_ALL=C.utf8 xterm -u8 -fa "$NERD_FONT" -fs 13 -bg "$bg" \
    -xrm "XTerm*internalBorder: 0" -xrm "XTerm*borderWidth: 0" \
    -geometry "${cols}x${rows}" -title "$title" \
    -e env XDG_CONFIG_HOME="$cfg_dir" XDG_DATA_HOME="$DATA/.." LANG=C.utf8 LC_ALL=C.utf8 "$BIN" \
    >/dev/null 2>&1 &
  local xterm_pid=$!
  sleep 1.2

  local win=""
  for _ in $(seq 1 30); do
    win="$(xdotool search --name "$title" 2>/dev/null | head -1)"
    [ -n "$win" ] && break
    sleep 0.2
  done
  if [ -z "$win" ]; then
    echo "warning: xterm window never appeared for $theme/$size_dir, skipping" >&2
    kill -9 "$xterm_pid" 2>/dev/null || true
    return
  fi
  xdotool windowfocus "$win"
  sleep 0.2

  send_text() { xdotool windowfocus "$win"; xdotool type --window "$win" --clearmodifiers -- "$1"; }
  send_key() { xdotool windowfocus "$win"; xdotool key --window "$win" --clearmodifiers "$1"; }
  shot() {
    mkdir -p "$OUT/$theme"
    sleep 0.35
    import -window "$win" "$OUT/$theme/$size_dir-$1.png"
  }

  if [ "$size_dir" = "wide" ]; then
    shot "01-notebooks"
    send_text "l"
    shot "02-notes"
    send_text "l"
    shot "03-preview"
    send_text "?"
    shot "04-which-key"
    send_key "Escape"
    send_text " c"
    shot "05-theme-picker"
    send_key "Escape"
    send_text " g"
    send_text "recipe"
    shot "06-global-search"
    send_key "Escape"
    send_text " T"
    shot "07-tags-panel"
    send_key "Escape"
    send_text "hD"
    shot "08-toggle-dates"
    send_text " l"
    shot "09-logs"
    send_key "Escape"
    send_text "T"
    shot "10-tree-view"
    send_key "Escape"
    send_text "lH"
    shot "11-history"
    send_key "Escape"
    send_text " U"
    sleep 1.5
    shot "12-check-update"
    send_key "Escape"
    send_text " b"
    shot "13-drawer"
    send_key "Escape"
    send_text " s"
    shot "14-settings"
    send_key "Escape"
    # /-menu: jump to the dedicated empty note (see setup_sample_data) by
    # title rather than by list position, so this doesn't depend on
    # exactly where it happens to sort among the other notes. `hhh` first
    # resets focus all the way back to NOTEBOOKS regardless of whatever
    # panel the steps above left focus on, since backward() is a no-op
    # once already there — cheap insurance against relying on incidental
    # leftover focus state.
    send_text "hhhl"
    send_text "/"
    send_text "quick"
    send_key "Return"
    send_text "i"
    send_text "/"
    shot "15-slash-menu"
    # Removes the "/" and exits without saving any real change — the note
    # must come back out exactly as empty as it went in, since setup_sample_data
    # only runs once and this same file is reused by every theme/tier capture
    # that follows.
    send_key "BackSpace"
    send_key "Escape"
    send_key "Escape"

    # EDITOR settings tab — GENERAL is the section a fresh leader+s always
    # opens on, so `Right` x3 (GENERAL -> THEME -> GIT -> EDITOR) reaches it
    # the same way a real user would, rather than assuming its index.
    send_text " s"
    send_key "Right"
    send_key "Right"
    send_key "Right"
    shot "16-settings-editor"
    send_key "Escape"

    # Multi-cursor + find/replace, on the dedicated errands-this-week note
    # (see setup_sample_data) — jumped to by title, same pattern as the
    # slash-menu note above.
    send_text "hhhl"
    send_text "/"
    send_text "errands"
    send_key "Return"
    send_text "i"
    shot "17-editor-line-numbers"
    # Ctrl+D 3x: 1st press selects the word under the cursor (no new
    # cursor yet), each press after that adds the next occurrence as its
    # own cursor — three identical "TODO"s means three presses select all
    # of them, with no pixel-coordinate mouse math or counted arrow-key
    # navigation needed at all.
    send_key "ctrl+d"
    send_key "ctrl+d"
    send_key "ctrl+d"
    shot "18-editor-multicursor-select"
    send_text "DONE"
    shot "19-editor-multicursor-typed"
    # `Escape` first collapses the secondary cursors (this app's own
    # convention) rather than leaving their now-stale positions/selections
    # around for the find/replace shot below. 4 Ctrl+U (one per typed
    # character, each grouped atomically across all 3 cursors — see
    # `editor_undo`'s own doc comment) restores "TODO" exactly, since this
    # note is shared by every theme/tier capture that follows.
    send_key "Escape"
    send_key "ctrl+u"
    send_key "ctrl+u"
    send_key "ctrl+u"
    send_key "ctrl+u"
    send_key "ctrl+f"
    send_text "milk"
    shot "20-editor-find-replace"
    send_key "Escape"
    send_key "Escape"
  else
    shot "overview"
  fi

  kill -9 "$xterm_pid" 2>/dev/null || true
  wait "$xterm_pid" 2>/dev/null || true
}

for theme in "${THEMES[@]}"; do
  echo "== $theme =="
  capture "$theme" wide 140 40
  capture "$theme" stacked 60 40
  capture "$theme" single 40 12
done

count=$(find "$OUT" -name '*.png' | wc -l)
echo "Done — $count screenshots in $OUT"
