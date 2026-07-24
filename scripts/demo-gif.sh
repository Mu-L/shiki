#!/usr/bin/env bash
# Generates docs/assets/demo.gif: a fast-paced, scripted tour of shiki
# against a deliberately rich dataset (3 notebooks, 30 notes, nested
# folders 2 levels deep, long note bodies, varied tags) — meant to show
# the app handling real volume, not a 3-note toy example.
#
# Uses VHS (https://github.com/charmbracelet/vhs): a `.tape` file is a
# literal, deterministic keystroke script, so the same recording comes out
# every run — no manual screen-recording/editing step, and it's cheap to
# re-run after every release against that release's own binary.
#
# Usage: scripts/demo-gif.sh [output-path]
#   Defaults to docs/assets/demo.gif.
#
# Requires (local dev machine or CI — see release.yml's update-screenshots
# job for the CI install list): vhs, ttyd, ffmpeg (vhs's own runtime deps),
# and a Nerd Font (same requirement as scripts/screenshots.sh, same
# fc-list auto-detection).
#
#   Arch/CachyOS:   sudo pacman -S vhs
#   Debian/Ubuntu:  see https://github.com/charmbracelet/vhs#installation
#     (vhs isn't in Ubuntu's default apt repos — install via the .deb
#     release asset or `go install`)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${1:-$ROOT/docs/assets/demo.gif}"
WORK="$(mktemp -d)"

cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT

for tool in vhs ttyd ffmpeg; do
  command -v "$tool" >/dev/null || {
    echo "error: $tool not found on \$PATH — see the header of this script for what's needed." >&2
    exit 1
  }
done

NERD_FONT="$(fc-list | grep -i "nerd font mono" | head -1 | cut -d: -f2 | sed 's/^ *//' | cut -d, -f1)"
NERD_FONT="${NERD_FONT:-monospace}"

BIN="$ROOT/target/release/shiki"
echo "Building release binary..."
cargo build --release -p shiki-cli --manifest-path "$ROOT/Cargo.toml"

# --- Rich sample data: 3 notebooks, 30 notes, folders 2 levels deep, long
# bodies, varied tags — deliberately more than scripts/screenshots.sh's
# minimal set, since this recording exists specifically to demonstrate
# shiki staying fast and legible at real volume, not just its layout.
DATA="$WORK/data/shiki"
CFG="$WORK/config/shiki"
mkdir -p "$DATA" "$CFG"

write_note() {
  local nb="$1" relpath="$2" title="$3" date="$4" tags="$5" body="$6"
  mkdir -p "$DATA/$nb/$(dirname "$relpath")"
  cat >"$DATA/$nb/$relpath" <<EOF
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

for nb in personal work research; do
  mkdir -p "$DATA/$nb"
  git -C "$DATA/$nb" init -q
  git -C "$DATA/$nb" config user.email "demo@shiki.dev"
  git -C "$DATA/$nb" config user.name "shiki demo"
done

# === personal/ — 8 root notes + journal/ (4) + projects/{shiki-app,website}/ (2 each) ===

write_note personal "book-recommendations.md" "Book recommendations" "2026-07-15" "[reading, books]" \
"## Currently reading

- *The Pragmatic Programmer* — Hunt & Thomas
- *Project Hail Mary* — Andy Weir

## Queue

- *A Philosophy of Software Design* — John Ousterhout
- *The Left Hand of Darkness* — Ursula K. Le Guin
- *Exhalation* — Ted Chiang
- *The Design of Everyday Things* — Don Norman

## Finished this year

1. *Klara and the Sun* — Kazuo Ishiguro — beautiful, quietly devastating.
2. *Debt: The First 5000 Years* — David Graeber — dense but worth it.
3. *The Phoenix Project* — Gene Kim — read this one for work, actually enjoyed it.

Recommended by a friend: anything by Ted Chiang, starting with *Exhalation*.
Also want to revisit *Gödel, Escher, Bach* — tried it in college, bounced off it,
might land better now."

write_note personal "gift-ideas.md" "Gift ideas" "2026-07-14" "[shopping, gifts]" \
"## Mom's birthday (August)

- That ceramic pour-over set she mentioned twice
- Framed print from the trip to the coast
- Gift card as backup if nothing lands in time

## Dad — Father's Day was already covered, but for Christmas

- Replacement head for the electric razor (he never buys this himself)
- A decent multitool — his current one is rusted through

## Housewarming (the Chens, new apartment)

- Something for the kitchen, they mentioned they don't own a real chef's knife
- A plant that's hard to kill — pothos or a snake plant

## Running list of \"good gift, no occasion yet\"

- Nice fountain pen
- A framed star chart for their anniversary
- That board game everyone keeps recommending"

write_note personal "morning-routine.md" "Morning routine" "2026-07-10" "[habits, productivity]" \
"## Current routine (roughly 6:15–7:30)

1. Wake up, no snooze — phone charges across the room specifically to force this.
2. Glass of water before coffee.
3. 20 minutes of reading (paper book, not a screen) with the first coffee.
4. Quick stretch/mobility routine — nothing fancy, maybe 10 minutes.
5. Shower, get dressed, out the door.

## What's actually working

- Reading before screens has been the single biggest change — sets a completely
  different tone for the day than opening a phone first thing.
- Water before coffee sounds trivial but noticeably helps with the mid-morning
  energy dip.

## What isn't

- The mobility routine gets skipped more often than not once things get busy.
  Might need to shrink it to something so short there's no excuse (5 minutes?).
- Still checking messages on the walk to the kitchen sometimes. Working on it.

## Experiment for next month

Try moving the workout to mornings instead of evenings, see if it sticks better
before the day has a chance to get away from me."

write_note personal "movie-watchlist.md" "Movie watchlist" "2026-07-08" "[entertainment]" \
"## To watch

- *Perfect Days* — heard it's meditative, in the mood for that
- *The Zone of Interest*
- *Past Lives*
- Rewatch *Paprika* — been too long

## Watched recently

- *Poor Things* — visually stunning, mixed on the pacing in the back half
- *Oppenheimer* — the sound design alone was worth the ticket
- *The Holdovers* — exactly the comfort-watch it looked like it'd be

## Someone's recommendation queue

Friend keeps insisting on the whole *Before* trilogy back to back. Need to
actually clear an evening for that instead of half-watching it in pieces."

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
6. Roll thin, cut to whatever shape — tagliatelle is the easiest to cut by hand.
7. Cook 2–3 minutes in heavily salted boiling water, fresh pasta cooks fast.

## Notes from last attempt

Dough was slightly too wet — next time start with 380g flour and add more only
if it won't come together, easier to add flour than take it away.

Best paired with a simple brown butter and sage sauce, or a light tomato and
basil if it's summer and the tomatoes are actually good."

write_note personal "travel-bucket-list.md" "Travel bucket list" "2026-07-05" "[travel, dreams]" \
"## Definitely happening (booked or budgeted)

- Portugal, October — Lisbon then a few days in Porto
- Long weekend in the mountains for the fall colors

## Someday list

- Japan in cherry blossom season — apparently you have to book almost a year out
- New Zealand, South Island specifically, for the hiking
- Slow train across a country instead of flying, just to see what that's like
- A proper long-haul backpacking trip before settling down feels harder to do

## Notes to self

Stop overplanning these and just book something. The Portugal trip almost
didn't happen because of exactly that — glad it did anyway."

write_note personal "weekend-hiking-trip.md" "Weekend hiking trip" "2026-07-10" "[outdoors, planning]" \
"Planning a two-day trip along the ridge trail.

## Logistics

- Trailhead parking fills up early, arrive before 7am
- Pack layers, weather can flip fast above 2000m
- Water refill point at the halfway hut
- Reserve the shelter two weeks in advance

## Gear checklist

- [ ] Tent + stakes
- [ ] Sleeping bag rated for the actual overnight low, not the daytime high
- [ ] Water filter, not just tablets
- [ ] Headlamp + spare batteries
- [ ] First aid kit, actually check what's in it before leaving

## Route notes from last time

The east approach is prettier but adds about two hours — worth it if the
weather holds, skip it if there's any chance of afternoon storms."

write_note personal "year-2026-goals.md" "2026 goals" "2026-01-02" "[goals, planning]" \
"## Health

- Run a half marathon — currently at 10k comfortably, need to build up
- Actually use the gym membership more than twice a month

## Work / craft

- Ship the side project instead of letting it rot in a private repo forever
- Get comfortable with a second language beyond what work requires

## Personal

- Read 24 books (2 a month) — tracking in book-recommendations.md
- Take the Portugal trip, don't let it slip like last year

## Mid-year check-in (July)

Running is going well, roughly on pace. Side project is... still not shipped.
Portugal is booked though, so that one's actually happening this time."

write_note personal "projects/shiki-app/roadmap.md" "shiki roadmap" "2026-07-01" "[rust, tui, project-management]" \
"## Shipped

- Three-pane Yazi-style layout, responsive to terminal size
- Notebooks as independent git repos
- Real per-note version history via git log
- 12 built-in themes, live picker
- In-TUI self-update

## In progress

- Marketing site + full documentation reference
- CI: automated screenshots + demo GIF per release

## Considering

- Shell completions + man page (clap_complete)
- Encrypted notes at rest — bigger scope, needs real design first
- Plugin system — probably not, keep the surface area small

## Explicitly not doing

- A GUI version — the whole point is staying in the terminal
- A hosted/cloud sync service — git remotes already solve this"

write_note personal "projects/shiki-app/bug-tracker.md" "shiki bug tracker" "2026-07-12" "[rust, tui, bugs]" \
"## Open

- Long lines in PREVIEW don't always wrap at a sensible point on very narrow
  terminals — check the wrapping logic against a 46-column window.
- Theme picker live-preview can lag by one frame on a very large notebook.

## Fixed recently

- White border in generated screenshots (xterm's internalBorder default) —
  fixed by setting -bg per theme and -xrm to zero the border out entirely.
- Theme screenshot not hiding behind the CSS fallback mockup — a CSS rule
  with higher specificity than the browser's default [hidden] rule.

## Won't fix

- Icons render as boxes without a Nerd Font installed — this is inherent to
  using Nerd Font glyphs at all, documented in the README prerequisites."

write_note personal "projects/website/todo.md" "Website TODO" "2026-07-20" "[web, project-management]" \
"## Before next release

- [ ] Regenerate screenshots against the new version
- [ ] Update the changelog section fetch to confirm it still parses cleanly
- [ ] Re-check the OG image still renders correctly after any copy changes

## Nice to have, not blocking

- [ ] Dark/light auto-detect based on the visitor's OS preference on first load
- [ ] A proper 404 page instead of GitHub Pages' default
- [ ] Analytics — privacy-respecting only, still deciding if it's worth it at all"

write_note personal "projects/website/design-notes.md" "Website design notes" "2026-07-19" "[web, design]" \
"## Why the theme switcher matters

Most landing pages for terminal tools show one static screenshot and call it
done. Letting a visitor actually click through the real palettes — with real
screenshots, not a CSS approximation — is a much stronger \"this is a real,
polished piece of software\" signal than any amount of copywriting.

## Layout decisions

- Themes section right after the hero, not buried after Features — the
  live-recolor effect should be visible within one scroll.
- Hero screenshot sized noticeably larger than the text column, since the
  screenshot itself is doing most of the persuading.

## Open questions

- Does the demo GIF belong in the hero too, replacing the static screenshot,
  or lower down as a supplement? Leaning toward: static screenshot in the
  hero (loads instantly), GIF further down where a visitor has already
  decided they're interested enough to keep scrolling."

write_note personal "journal/2026-07-20.md" "2026-07-20" "2026-07-20" "[journal]" \
"Spent most of today heads-down on the release checklist. Longer than expected
because of the screenshot border bug — small thing, but the fix took a while
to actually track down to the right root cause instead of just papering over
the symptom.

Good focus day overall. Ended with a short walk, needed it after staring at a
terminal for that long."

write_note personal "journal/2026-07-21.md" "2026-07-21" "2026-07-21" "[journal]" \
"Slower day. Spent the morning on something that turned out to be a dead end —
tried optimizing a code path that wasn't actually the bottleneck. Should have
measured first instead of assuming.

Lesson, again: profile before optimizing, no matter how obvious the \"obvious\"
bottleneck seems."

write_note personal "journal/2026-07-22.md" "2026-07-22" "2026-07-22" "[journal]" \
"Good conversation about the roadmap today. Landed on: ship what's already
built and polished before starting anything new, rather than letting three
half-finished features pile up at once.

Also finally fixed the thing that's been bugging me about the theme picker's
live preview lag. Turned out to be a caching issue, of course it was."

write_note personal "journal/2026-07-23.md" "2026-07-23" "2026-07-23" "[journal]" \
"Big day — the whole open-source prep landed: contributing guide, code of
conduct, security policy, issue templates, branch protection, the works.
Also the marketing site went live.

Satisfying to see it all actually deployed instead of sitting as local
commits. Tomorrow: keep an eye on whether any of the automation breaks on
the next real release."

# === work/ — 5 root notes + clients/ (2) + docs/ (2) ===

write_note work "meeting-notes-q3-planning.md" "Meeting notes: Q3 planning" "2026-07-20" "[work, meetings, planning]" \
"## Attendees

Product, Engineering, Design leads.

## Decisions

- Ship the notebook tree view before the Q3 review
- Push the mobile companion app to Q4 — not enough bandwidth to do it well
- Design to finalize the onboarding flow by end of month

## Action items

- [ ] Engineering: scope the tree view work, rough estimate by Friday
- [ ] Design: onboarding flow mockups
- [ ] Product: update the roadmap doc to reflect the Q4 push

## Open questions carried to next meeting

Do we need a dedicated mobile design pass before Q4, or can engineering start
from the desktop flows and adapt as they go?"

write_note work "architecture-decisions.md" "Architecture decisions" "2026-07-05" "[work, architecture]" \
"## ADR-014: Background sync via a dedicated thread + channel

Chose a plain std::thread + mpsc::channel over pulling in an async runtime,
since the rest of the render loop is already a synchronous ~100ms poll loop.
Adding async for one feature would be inconsistent with everything else and
buy nothing.

## ADR-015: Per-notebook sync policy overrides

Global git settings weren't enough — a private work repo should auto-push,
a scratch notebook with no remote shouldn't try to sync at all. Solved with
an optional override table per notebook, falling back to global defaults
for anything unset.

## ADR-016: Real screenshot generation over hand-drawn mockups

For any UI documentation/marketing asset, prefer capturing the actual
running app over a hand-drawn approximation — mockups drift from reality,
real screenshots can't."

write_note work "sprint-review.md" "Sprint review" "2026-07-18" "[work, retro]" \
"## Completed

- Folder move/copy/delete, generalized addressing scheme
- Visual mode multi-select, wired up after sitting dead in the enum for a while
- Auto-tag workflow, closes the gap where a release could be built but never
  actually get tagged/published

## Carried over

- Marketing site polish — bigger than expected once the theme switcher and
  live changelog were added on top of the original plan

## Retro notes

Estimation was off on \"add a marketing site\" — treated it like a small task,
it grew into: theme switcher, screenshot automation, SEO, a full documentation
page, and a CI deploy pipeline. Should have scoped it as its own sprint from
the start instead of squeezing it in."

write_note work "onboarding-checklist.md" "Onboarding checklist" "2026-06-01" "[work, onboarding]" \
"## Day one

- [ ] Repo access, CI secrets explained (what's provisioned, what's pending)
- [ ] Walk through CLAUDE.md — it's the actual source of truth for
  non-obvious decisions, not just a formality
- [ ] Get the dev environment building: cargo check --workspace should be
  green before anything else

## First week

- [ ] Pick a small, well-scoped issue to get a full PR through the pipeline
  once, branch protection and all
- [ ] Read through the four-crate architecture split — shiki-core has no
  ratatui dependency on purpose, don't reach for it there

## Common first-week mistakes to flag early

- Adding a TUI-only concept into shiki-core (it should stay pure domain logic)
- Reaching for git commands directly instead of the existing git.rs helpers"

write_note work "team-retro-notes.md" "Team retro notes" "2026-07-01" "[work, retro]" \
"## What went well

- Shipping in small, reviewable increments instead of one giant PR
- Documenting the *why* behind non-obvious decisions as we go, not after
  the fact when the reasoning's already been forgotten

## What didn't

- A couple of \"quick fixes\" turned out to have a root cause worth actually
  investigating instead of patching the symptom — cost more time overall
  than just investigating properly the first time would have

## Action for next sprint

When something feels like it should be quick and isn't, that's the signal
to stop and actually understand it rather than pushing through."

write_note work "clients/acme-corp.md" "Acme Corp" "2026-06-15" "[work, clients]" \
"## Contacts

- Primary: their eng lead, prefers async updates over calls
- Escalation: their PM, only loop in for anything actually blocking

## Current engagement

Quarterly infrastructure review, mostly advisory. Nothing time-sensitive
right now, next check-in scheduled for end of quarter.

## History

Been a client for two years, generally low-friction. One rough patch early
on around unclear scope — fixed by writing everything into a shared doc
before starting future engagements."

write_note work "clients/globex-industries.md" "Globex Industries" "2026-05-20" "[work, clients]" \
"## Contacts

- Primary: their CTO, very hands-on, expect direct technical questions
- They prefer everything in writing — document decisions, don't rely on
  verbal agreements from calls

## Current engagement

Migration project, roughly 60% complete. On schedule as of the last check-in.

## Watch items

- Their legacy system's undocumented edge cases keep surfacing later than
  ideal — worth over-communicating discovery of these as they're found
  rather than batching them into a single end-of-phase report."

write_note work "docs/api-reference.md" "API reference" "2026-07-10" "[work, docs]" \
"## Authentication

All requests require a bearer token in the Authorization header.
Tokens are scoped per-client, rotate every 90 days.

## Endpoints

- GET /v1/status — health check, no auth required
- GET /v1/resources — list resources, paginated
- POST /v1/resources — create, idempotent via a client-supplied key
- DELETE /v1/resources/{id} — soft-delete, recoverable for 30 days

## Rate limits

1000 requests/hour per token, 429 with a Retry-After header when exceeded."

write_note work "docs/deployment-guide.md" "Deployment guide" "2026-07-08" "[work, docs, deployment]" \
"## Pre-deploy checklist

- [ ] All tests green on the target branch
- [ ] Changelog entry added
- [ ] Database migrations reviewed if any are included

## Deploy steps

1. Tag the release
2. CI builds and runs the full verification suite
3. Manual approval gate for production (deliberately not fully automatic)
4. Rollout is gradual, monitored at each stage before proceeding

## Rollback

Previous version stays deployable for 48 hours after any release
specifically so a rollback is always a known-good, already-tested target."

# === research/ — 5 root notes, no folders (a flat notebook for variety) ===

write_note research "rust-async-patterns.md" "Rust async patterns" "2026-07-02" "[rust, research]" \
"## When NOT to reach for async

If the rest of a codebase is already synchronous (a plain render loop, a
CLI tool with no concurrent I/O to overlap), pulling in an async runtime for
one feature is usually the wrong call — a plain thread + channel handles
\"do this in the background, tell me when it's done\" just fine without the
added complexity of a runtime nobody else in the codebase uses.

## When it's clearly worth it

- Genuinely concurrent I/O — many sockets/requests in flight at once
- A framework that's already async-first (most web servers)

## Patterns worth remembering

- mpsc channels for background-thread-to-main-thread communication cover a
  surprising amount of what people reach for async for
- Capturing values *before* a long-running operation that might invalidate
  them (e.g. current_exe() before self-replacing the running binary) is a
  general pattern, not async-specific, but bites people in async code a lot"

write_note research "tui-design-inspiration.md" "TUI design inspiration" "2026-06-20" "[tui, research, design]" \
"## Yazi

The Miller-columns file manager layout (each level collapses the previous
one down to a thin strip) translates surprisingly well to a notes app —
notebooks/notes/preview maps naturally onto the same pattern file managers
already use for directories/files/preview.

## Helix

Modal editing done right — the which-key-style discoverability (press a
key, see what it could lead to) lowers the learning curve a lot compared to
expecting users to memorize a keybinding cheat sheet up front.

## Common thread across the good ones

Consistent, hardcoded navigation (movement keys behave the same everywhere)
combined with fully custom action keybindings per context. Trying to make
navigation itself configurable seems to be a trap — it stops feeling
predictable the moment two similar apps bind movement differently."

write_note research "note-taking-apps-comparison.md" "Note-taking apps comparison" "2026-06-25" "[research, notes-apps]" \
"## Obsidian

Excellent linking/graph view, but Electron-based — heavier than it needs to
be for what's fundamentally text editing, and the vault format, while
plain-text, has enough proprietary conventions layered on top that it
doesn't feel fully portable.

## Notion

Powerful, but genuinely proprietary storage — no meaningful offline story,
and exporting cleanly is harder than it should be for something calling
itself a notes app.

## Plain directories of Markdown + a tool on top (nb, this project)

The actual notes are just files, always portable, always readable with
nothing but a text editor even if the tool itself disappeared. The tradeoff
is building (or choosing) a tool that makes that plain-file approach
pleasant to use day to day, instead of getting linking/search/organization
for free from a database-backed app."

write_note research "git-internals.md" "Git internals" "2026-06-10" "[git, research]" \
"## Objects

Everything is a blob, tree, commit, or tag — content-addressed by SHA. A
commit is really just a pointer to a tree plus some metadata and a pointer
to its parent(s).

## Why per-file history \"for free\" isn't actually free

git has no built-in \"log for one path\" primitive at the object level —
\`git log -- path\` (and shiki's own file_history) has to walk the full
commit graph and diff each commit's tree against its parent's, checking
whether the specific path's blob changed. It's not indexed by path anywhere
in the object model itself.

## Fast-forward vs. real merges

A fast-forward is just moving a branch pointer forward when the current
branch's history is a strict subset of the target — no new commit is
created. This is why \`pull\`'s fast-forward-or-fail behavior can't just
always succeed: if local history has diverged, git has no fast-forward
path available at all, only a real merge or a rebase."

write_note research "terminal-ui-libraries.md" "Terminal UI libraries" "2026-06-05" "[rust, tui, research]" \
"## ratatui

Immediate-mode-ish rendering — you rebuild the widget tree every frame from
current state, and the library diffs against the previous frame's buffer to
compute the minimal set of terminal writes. No persistent widget objects to
manage state for.

## crossterm

The terminal backend underneath — raw mode, alternate screen, mouse
capture, cross-platform (including real Windows console support, not just
ANSI-passthrough), which is why ratatui pairs with it instead of a
Unix-only backend.

## Why not a retained-mode GUI toolkit's terminal equivalent

Retained-mode makes sense when widget state is expensive to rebuild every
frame. For a note-taking TUI redrawing at ~10Hz against small in-memory
lists, immediate-mode's simplicity (state lives in one place, rendering is
a pure function of it) outweighs any performance argument for retained
widgets."

echo "Sample data written under $DATA"

# --- Config: a real theme active, matching the version currently being
# recorded, plus enough of a keybindings/git setup that the demo doesn't
# hit any first-run prompts.
mkdir -p "$CFG"
cat >"$CFG/config.toml" <<EOF
[general]
default_notebook = "personal"

[theme]
name = "gruvbox-dark"

[git]
auto_commit = false
auto_push = false
EOF

# --- Commit everything so the footer shows a clean synced state instead of
# a distracting dirty marker throughout the recording.
for nb in personal work research; do
  git -C "$DATA/$nb" add -A
  git -C "$DATA/$nb" commit -q -m "seed demo data"
done

mkdir -p "$(dirname "$OUT")"

TAPE="$WORK/demo.tape"
cat >"$TAPE" <<TAPEEOF
Output "$OUT"
Set Shell "bash"
Set FontFamily "$NERD_FONT"
Set FontSize 15
Set Width 1200
Set Height 750
Set Padding 0
Set TypingSpeed 30ms
Set WaitTimeout 5s

Hide
Type "XDG_CONFIG_HOME='$CFG' XDG_DATA_HOME='$WORK/data' '$BIN'"
Enter
Sleep 1200ms
Show

# --- Phase 1: notebooks tour (NOTEBOOKS focus, cursor starts on
# alphabetically-first "personal"; NotebookStore::list sorts by name, same
# as list_dir does for folders/notes within one — confirmed against
# shiki-core/src/notebook.rs before writing this, not assumed).
Sleep 500ms
Down@150ms 2
Sleep 300ms
Up@150ms 2
Sleep 400ms
Enter

# --- Phase 2: browse personal's root list fast, open a note with real
# content, scroll it, then explicitly return to NOTES (Left, not Escape —
# Escape only closes popups/cancels leader, it does NOT move focus out of
# PREVIEW; only h/Left does).
Sleep 400ms
Down@100ms 4
Sleep 300ms
Enter
Sleep 500ms
Down@100ms 6
Sleep 600ms
Left

# --- Phase 3: descend two folder levels deep, open a note there, then
# ascend all the way back out (Left ascends one folder level at a time
# before finally falling back to switching focus, once at notebook root).
# Cursor is at index 4 ("Morning routine") after Phase 2's Left — VHS has
# no dedicated Home/End key command, so this moves back to index 0
# (journal/) the same way End-of-Phase-2's Down got there, just reversed.
Up@150ms 4
Sleep 300ms
Down@150ms 1
Sleep 300ms
Enter
Sleep 500ms
Enter
Sleep 500ms
Down@150ms 1
Sleep 300ms
Enter
Sleep 900ms
Left
Sleep 300ms
Left
Sleep 300ms
Left
Sleep 400ms

# --- Phase 4: fuzzy jump within the notebook. `PendingInput::Search`'s
# confirm handler (app.rs) sets `self.focus = Focus::Notes` directly —
# unlike the tags/global-search jumps, it does NOT land in PREVIEW — so
# no extra `Left` is needed (or wanted) here. An earlier version of this
# script added one anyway on the wrong assumption it behaved like those
# other jumps; since we're already back at notebook root at this point
# (the jumped-to note has no parent folder), that stray `Left` — with
# focus already on NOTES and notes_path already empty — fell through to
# `navigate_backward`'s panel-switch fallback and silently flipped focus
# to NOTEBOOKS for the rest of the recording (invisible for several
# phases since leader-bound modals work regardless of focus, until
# Phase 7's Down@150ms 4 exposed it as a wraparound notebook jump).
Type "/"
Sleep 300ms
Type "hiking"
Sleep 700ms
Enter
Sleep 900ms

# --- Phase 5: global fuzzy search across every notebook — a leader
# binding, reachable regardless of current focus.
Space
Type "g"
Sleep 400ms
Type "roadmap"
Sleep 700ms
Escape
Sleep 600ms

# --- Phase 6: tags panel — two levels deep (tag list, then notes carrying
# it); one Escape only backs out of level 2, a second is needed to fully
# close the modal. Extra settle time (600ms, not 300ms) after each Escape
# here — this transition is where a flaky run once let a stray keystroke
# get misdelivered to the wrong modal (a still-active overlay eating a key
# meant for the next phase), so every modal close in this script now gets
# a full render-loop cycle (shiki polls at ~100ms) plus margin to actually
# settle before the next keystroke fires.
Space
Type "T"
Sleep 500ms
Down@150ms 3
Sleep 500ms
Enter
Sleep 700ms
Escape
Sleep 600ms
Escape
Sleep 600ms

# --- Phase 7: visual mode multi-select — a notes-scope action, so NOTES
# must actually be focused (it is, after phase 6 closed back to it).
Type "v"
Sleep 500ms
Down@150ms 4
Sleep 700ms
Escape
Sleep 600ms

# --- Phase 8: theme picker, live-cycle a few palettes fast — another
# leader binding, reachable regardless of focus.
Space
Type "c"
Sleep 500ms
Down@200ms 3
Sleep 900ms
Escape
Sleep 500ms

# --- Phase 9: which-key / command palette, quick glimpse.
Type "?"
Sleep 900ms
Escape
Sleep 900ms

# Quit off-screen — ending on the app itself, not a bare shell prompt with
# the XDG_CONFIG_HOME/XDG_DATA_HOME launch command sitting there.
Hide
Type "q"
Sleep 300ms
TAPEEOF

echo "Recording with VHS..."
vhs "$TAPE"

echo "Wrote $OUT"
