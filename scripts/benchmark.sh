#!/usr/bin/env bash
# Automated CPU / memory / responsiveness benchmark for the shiki TUI.
#
# Two kinds of measurement, both from real, kernel-reported numbers for the
# actual running binary — nothing here is estimated or simulated:
#
#   1. Idle cost: utime+stime (from /proc/<pid>/stat, in clock ticks) and
#      VmRSS (from /proc/<pid>/status) sampled while the app sits idle in a
#      representative state, at the same ~100ms poll/draw cadence as
#      `shiki-tui/src/app.rs`'s `run()`.
#   2. Responsiveness: wall-clock time from process spawn until the expected
#      content actually appears in the terminal (`wait_for_pattern`, polling
#      `tmux capture-pane` every 100ms) — this is what a freeze/hang would
#      show up as. For scenarios with a navigation step (`deep-nesting`),
#      the same technique times that step too.
#
# Scenarios, small to large — `big-folder`/`huge-note` are the ordinary
# cases; the `*-100k`/`*-massive`/`deep-nesting` ones are deliberately
# extreme (see the CLI's own note count in real use for comparison) to
# prove the caches introduced in `App` (`folder_preview_cache`,
# `note_preview_cache`) don't just help at moderate scale but keep the app
# responsive at sizes far beyond normal use:
#   baseline          empty notebook — floor cost of the render loop itself.
#   typical-notes     20 short notes at the root — ordinary everyday use.
#   big-folder        one subfolder, $BIG_FOLDER_COUNT notes, selected but
#                      not entered (PREVIEW's folder-peek).
#   big-folder-100k   same, at $HUGE_FOLDER_COUNT notes.
#   huge-note         one ~$HUGE_NOTE_LINES-line note, selected (PREVIEW's
#                      markdown_to_lines).
#   huge-note-massive same, at $MASSIVE_NOTE_LINES lines.
#   deep-nesting      $DEEP_LEVELS nested folders, one per level, descended
#                      one at a time (`l`) down to a leaf folder with real
#                      notes in it.
#
# Usage: scripts/benchmark.sh [duration_seconds]
#   duration_seconds: how long to sample each scenario's idle cost, once it
#   has actually finished loading (default 12).
#
# Requires: tmux (drives the TUI headless, same technique used to verify
# behavior manually — see CLAUDE.md's "verify live via tmux" convention). A
# release build is produced automatically if missing/stale; debug builds are
# deliberately not used here since they'd overstate real-world CPU cost.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DURATION="${1:-12}"
WORK="$(mktemp -d)"
HZ="$(getconf CLK_TCK)"
TMUX_PREFIX="shiki-bench-$$"
RAW_TSV="$WORK/raw_results.tsv"

SMALL_FOLDER_COUNT=20
BIG_FOLDER_COUNT=3000
HUGE_FOLDER_COUNT=100000
HUGE_NOTE_LINES=20000
MASSIVE_NOTE_LINES=300000
DEEP_LEVELS=200
DEEP_LEAF_NOTES=50
READY_TIMEOUT=90 # seconds — generous, since this is exactly what we're measuring

cleanup() {
  tmux list-sessions -F '#{session_name}' 2>/dev/null | grep "^${TMUX_PREFIX}-" | while read -r s; do
    tmux kill-session -t "$s" 2>/dev/null || true
  done
  rm -rf "$WORK"
}
trap cleanup EXIT

command -v tmux >/dev/null || {
  echo "error: tmux not found on \$PATH — required to drive the TUI headlessly." >&2
  exit 1
}

echo "Building release binary (debug builds run notably slower and would overstate CPU cost)..."
cargo build --release -p shiki-cli --manifest-path "$ROOT/Cargo.toml" >/dev/null
BIN="$ROOT/target/release/shiki"

echo "Sampling ${DURATION}s of idle cost per scenario, after it actually finishes loading. HZ=$HZ (1 clock tick = $(awk -v hz="$HZ" 'BEGIN{printf "%.1f", 1000/hz}') ms)."
echo

# --- Scenario data generation ----------------------------------------------
# All bulk generation goes through awk (one process opening/writing/closing
# each file) rather than a bash loop spawning `cat`/redirection per file —
# at $HUGE_FOLDER_COUNT scale a per-file subshell is the bottleneck, not the
# actual write: generating 100,000 tiny files this way takes well under a
# second, vs. tens of seconds forking a process per file.
gen_notes() {
  local dir="$1" count="$2" prefix="$3"
  mkdir -p "$dir"
  awk -v dir="$dir" -v count="$count" -v prefix="$prefix" '
    BEGIN {
      for (i = 1; i <= count; i++) {
        f = dir "/" prefix "-" i ".md"
        print "---" > f
        print "title: " prefix " note " i >> f
        print "date: 2026-01-01" >> f
        print "tags: []" >> f
        print "notebook: bench" >> f
        print "---" >> f
        print "" >> f
        print "Body text for " prefix " note " i "." >> f
        close(f)
      }
    }'
}

gen_huge_note() {
  local path="$1" lines="$2"
  {
    printf '%s\n' "---" "title: Huge note" "date: 2026-01-01" "tags: []" "notebook: bench" "---" ""
    awk -v n="$lines" 'BEGIN{for(i=0;i<n;i++) print "- item " i ": lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor"}'
  } >"$path"
}

# A chain of $depth folders, one nested inside the previous, with real notes
# only at the bottom — exercises `Notebook::list_dir` at depth, not just at
# the root (a notebook can nest arbitrarily deep, per shiki-core's design).
gen_deep_chain() {
  local base="$1" depth="$2" leaf_notes="$3"
  local path="$base"
  for i in $(seq 1 "$depth"); do
    path="$path/level$i"
  done
  gen_notes "$path" "$leaf_notes" leaf
}

# Single notebook named "bench" per scenario dir — with exactly one
# notebook, App defaults `selected_notebook` to it and lands on its root
# with the first folder/note already selected, so most scenarios need zero
# navigation keystrokes (removes timing-dependent key-sending as a source
# of flakiness, except for `deep-nesting` which explicitly wants to time
# navigation).
new_scenario_dirs() {
  local name="$1"
  local data="$WORK/$name/data"
  local cfg="$WORK/$name/config"
  mkdir -p "$data/shiki/bench" "$cfg"
  (cd "$data/shiki/bench" && git init -q && git config user.email bench@shiki.dev && git config user.name bench)
  echo "$data" "$cfg"
}

# --- Responsiveness: wait for a literal string to actually appear in the
# rendered terminal, polling every 100ms (same cadence as the app's own
# poll loop) up to $3 seconds. Prints elapsed seconds; this elapsed time
# *is* the freeze/hang measurement — a scenario that's still synchronously
# parsing 100k files shows up here as a large number, not as CPU% (which
# only starts being sampled once we already know the app is ready).
wait_for_pattern() {
  local session="$1" pid="$2" pattern="$3" timeout="$4"
  local t0 t1
  t0=$(date +%s.%N)
  local tries=$((timeout * 10))
  local i=0
  while [ "$i" -lt "$tries" ]; do
    if tmux capture-pane -t "$session" -p 2>/dev/null | grep -qF "$pattern"; then
      t1=$(date +%s.%N)
      awk -v a="$t0" -v b="$t1" 'BEGIN{printf "%.2f", b-a}'
      return 0
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
      echo "-1"
      return 1
    fi
    sleep 0.1
    i=$((i + 1))
  done
  echo "TIMEOUT>${timeout}s"
  return 1
}

# --- Measurement -------------------------------------------------------------
# $4 ready_pattern: string to wait for right after startup (this scenario's
#   "has it actually finished loading" check).
# $5/$6 (optional): nav_keys / ready_pattern2 — for scenarios with a
#   navigation step to also time (only `deep-nesting` uses this).
measure_scenario() {
  local name="$1" data_root="$2" cfg_root="$3" ready_pattern="$4" nav_keys="${5:-}" ready_pattern2="${6:-}"
  local session="${TMUX_PREFIX}-${name}"

  tmux new-session -d -s "$session" -x 200 -y 50 \
    "XDG_CONFIG_HOME=$cfg_root XDG_DATA_HOME=$data_root $BIN"

  local pid=""
  for _ in $(seq 1 50); do
    pid="$(pgrep -f "^${BIN}\$" | head -1 || true)"
    [ -n "$pid" ] && break
    sleep 0.2
  done
  if [ -z "$pid" ]; then
    echo "warning: $name never started, skipping" >&2
    tmux kill-session -t "$session" 2>/dev/null || true
    return
  fi

  local first_frame_s
  first_frame_s="$(wait_for_pattern "$session" "$pid" "$ready_pattern" "$READY_TIMEOUT")"
  if [[ "$first_frame_s" == TIMEOUT* || "$first_frame_s" == "-1" ]]; then
    echo "warning: $name never showed '$ready_pattern' ($first_frame_s) — treating as a freeze, skipping idle sample" >&2
    printf '%s\t%s\t%s\t%d\t%s\t%s\t%s\t%s\t%s\t%s\t%d\n' \
      "$name" "$first_frame_s" "0.00" "$DURATION" "n/a" "n/a" "n/a" "n/a" "n/a" "n/a" 0 >>"$RAW_TSV"
    tmux kill-session -t "$session" 2>/dev/null || true
    kill -9 "$pid" 2>/dev/null || true
    return
  fi

  local nav_s="0.00"
  if [ -n "$nav_keys" ]; then
    tmux send-keys -t "$session" -l "$nav_keys"
    nav_s="$(wait_for_pattern "$session" "$pid" "$ready_pattern2" "$READY_TIMEOUT")"
    if [[ "$nav_s" == TIMEOUT* || "$nav_s" == "-1" ]]; then
      echo "warning: $name navigation never showed '$ready_pattern2' ($nav_s) — treating as a freeze, skipping idle sample" >&2
      printf '%s\t%s\t%s\t%d\t%s\t%s\t%s\t%s\t%s\t%s\t%d\n' \
        "$name" "$first_frame_s" "$nav_s" "$DURATION" "n/a" "n/a" "n/a" "n/a" "n/a" "n/a" 0 >>"$RAW_TSV"
      tmux kill-session -t "$session" 2>/dev/null || true
      kill -9 "$pid" 2>/dev/null || true
      return
    fi
  fi

  local u0 s0 rss0 cs0
  read -r u0 s0 < <(awk '{print $14, $15}' "/proc/$pid/stat")
  rss0="$(awk '/VmRSS/{print $2}' "/proc/$pid/status")"
  cs0="$(awk -F: '/^voluntary_ctxt_switches/{gsub(/[ \t]/,"",$2); print $2}' "/proc/$pid/status")"
  local t0 rss_max=$rss0 elapsed=0
  t0=$(date +%s)

  while [ "$elapsed" -lt "$DURATION" ]; do
    sleep 1
    elapsed=$((elapsed + 1))
    if ! kill -0 "$pid" 2>/dev/null; then
      echo "warning: $name process exited mid-sample (after ${elapsed}s)" >&2
      break
    fi
    local rss_now
    rss_now="$(awk '/VmRSS/{print $2}' "/proc/$pid/status" 2>/dev/null || echo "$rss_max")"
    [ "$rss_now" -gt "$rss_max" ] && rss_max=$rss_now
  done

  local t1 u1 s1 rss1 cs1
  t1=$(date +%s)
  read -r u1 s1 < <(awk '{print $14, $15}' "/proc/$pid/stat" 2>/dev/null || echo "$u0 $s0")
  rss1="$(awk '/VmRSS/{print $2}' "/proc/$pid/status" 2>/dev/null || echo "$rss0")"
  cs1="$(awk -F: '/^voluntary_ctxt_switches/{gsub(/[ \t]/,"",$2); print $2}' "/proc/$pid/status" 2>/dev/null || echo "$cs0")"

  tmux kill-session -t "$session" 2>/dev/null || true
  kill -9 "$pid" 2>/dev/null || true

  local real_elapsed=$((t1 - t0))
  [ "$real_elapsed" -le 0 ] && real_elapsed=1
  local ticks=$(((u1 - u0) + (s1 - s0)))
  local loop_iters=$((cs1 - cs0))
  local cpu_pct rss_start_mb rss_end_mb rss_max_mb rss_drift_mb
  cpu_pct=$(awk -v t="$ticks" -v hz="$HZ" -v e="$real_elapsed" 'BEGIN{printf "%.3f", (t/hz)/e*100}')
  rss_start_mb=$(awk -v k="$rss0" 'BEGIN{printf "%.2f", k/1024}')
  rss_end_mb=$(awk -v k="$rss1" 'BEGIN{printf "%.2f", k/1024}')
  rss_max_mb=$(awk -v k="$rss_max" 'BEGIN{printf "%.2f", k/1024}')
  rss_drift_mb=$(awk -v a="$rss0" -v b="$rss1" 'BEGIN{printf "%.2f", (b-a)/1024}')

  printf '%s\t%s\t%s\t%d\t%s\t%s\t%s\t%s\t%s\t%s\t%d\n' \
    "$name" "$first_frame_s" "$nav_s" "$real_elapsed" "$cpu_pct" "$ticks" "$rss_start_mb" "$rss_end_mb" "$rss_max_mb" "$rss_drift_mb" "$loop_iters" \
    >>"$RAW_TSV"
}

# --- Build scenarios and run them, one at a time (never concurrently — two
# benched processes competing for the CPU would corrupt every measurement).

read -r baseline_data baseline_cfg < <(new_scenario_dirs baseline)
echo "-> baseline (empty notebook)"
measure_scenario baseline "$baseline_data" "$baseline_cfg" "bench"

read -r typical_data typical_cfg < <(new_scenario_dirs typical-notes)
gen_notes "$typical_data/shiki/bench" "$SMALL_FOLDER_COUNT" note
echo "-> typical-notes ($SMALL_FOLDER_COUNT notes)"
measure_scenario typical-notes "$typical_data" "$typical_cfg" "bench"

read -r big_data big_cfg < <(new_scenario_dirs big-folder)
gen_notes "$big_data/shiki/bench/data" "$BIG_FOLDER_COUNT" note
echo "-> big-folder ($BIG_FOLDER_COUNT notes in one subfolder, selected but not entered)"
measure_scenario big-folder "$big_data" "$big_cfg" "note note 1"

read -r hugefolder_data hugefolder_cfg < <(new_scenario_dirs big-folder-100k)
gen_notes "$hugefolder_data/shiki/bench/data" "$HUGE_FOLDER_COUNT" note
echo "-> big-folder-100k ($HUGE_FOLDER_COUNT notes in one subfolder, selected but not entered)"
measure_scenario big-folder-100k "$hugefolder_data" "$hugefolder_cfg" "note note 1"

read -r huge_data huge_cfg < <(new_scenario_dirs huge-note)
gen_huge_note "$huge_data/shiki/bench/huge.md" "$HUGE_NOTE_LINES"
echo "-> huge-note (~$HUGE_NOTE_LINES-line note)"
measure_scenario huge-note "$huge_data" "$huge_cfg" "item 0:"

read -r massive_data massive_cfg < <(new_scenario_dirs huge-note-massive)
gen_huge_note "$massive_data/shiki/bench/huge.md" "$MASSIVE_NOTE_LINES"
echo "-> huge-note-massive (~$MASSIVE_NOTE_LINES-line note)"
measure_scenario huge-note-massive "$massive_data" "$massive_cfg" "item 0:"

read -r deep_data deep_cfg < <(new_scenario_dirs deep-nesting)
gen_deep_chain "$deep_data/shiki/bench" "$DEEP_LEVELS" "$DEEP_LEAF_NOTES"
echo "-> deep-nesting ($DEEP_LEVELS nested folders, descended one at a time down to $DEEP_LEAF_NOTES notes)"
nav_keys=""
for _ in $(seq 1 "$DEEP_LEVELS"); do nav_keys="${nav_keys}l"; done
measure_scenario deep-nesting "$deep_data" "$deep_cfg" "level1" "$nav_keys" "leaf note 1"

echo

# --- Report -----------------------------------------------------------------

if [ ! -s "$RAW_TSV" ]; then
  echo "No scenario produced a result — nothing to report." >&2
  exit 1
fi

printf '%-18s %10s %8s %8s %10s %10s %10s %10s %10s %8s\n' \
  "scenario" "1st_frame" "nav_s" "cpu%" "rss_start" "rss_end" "rss_max" "rss_drift" "secs" "loop/s"
printf '%-18s %10s %8s %8s %10s %10s %10s %10s %10s %8s\n' \
  "--------" "---------" "-----" "----" "---------" "-------" "-------" "---------" "----" "------"
while IFS=$'\t' read -r name frame nav secs cpu ticks rss_s rss_e rss_m rss_d iters; do
  if [ "$rss_s" = "n/a" ]; then
    printf '%-18s %9ss %7ss %8s %10s %10s %10s %10s %10s %8s\n' \
      "$name" "$frame" "$nav" "FROZE" "-" "-" "-" "-" "-" "-"
    continue
  fi
  loops_per_sec=$(awk -v i="$iters" -v s="$secs" 'BEGIN{printf "%.1f", i/s}')
  printf '%-18s %9ss %7ss %7s%% %9sM %9sM %9sM %9sM %9ss %8s\n' \
    "$name" "$frame" "$nav" "$cpu" "$rss_s" "$rss_e" "$rss_m" "$rss_d" "$secs" "$loops_per_sec"
done <"$RAW_TSV"

echo
echo "Columns: 1st_frame = wall-clock seconds from process spawn until the"
echo "         scenario's expected content actually rendered — this is the"
echo "         freeze/hang measurement, not an estimate. nav_s = same, timing"
echo "         the navigation step (deep-nesting only; 0.00s elsewhere)."
echo "         cpu% = (utime+stime ticks over the idle window)/HZ/secs*100,"
echo "         sampled only *after* the scenario finished loading. rss_* in"
echo "         MB (VmRSS); rss_drift = end-start over the idle window — a"
echo "         sustained positive drift across repeated runs is what an"
echo "         actual leak would look like. loop/s = voluntary_ctxt_switches"
echo "         delta/secs, a proxy for render-loop iterations (~10/s"
echo "         expected, since the loop blocks in event::poll(100ms))."
echo

echo "{"
first=1
while IFS=$'\t' read -r name frame nav secs cpu ticks rss_s rss_e rss_m rss_d iters; do
  [ "$first" -eq 1 ] || echo ","
  first=0
  printf '  "%s": {"first_frame_s": "%s", "nav_s": "%s", "seconds": %s, "cpu_percent": %s, "cpu_ticks": %s, "rss_start_mb": "%s", "rss_end_mb": "%s", "rss_max_mb": "%s", "rss_drift_mb": "%s", "loop_iters": %s}' \
    "$name" "$frame" "$nav" "$secs" "$cpu" "$ticks" "$rss_s" "$rss_e" "$rss_m" "$rss_d" "$iters"
done <"$RAW_TSV"
echo
echo "}"
