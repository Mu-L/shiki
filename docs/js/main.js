// Theme list mirrors shiki-config/src/themes/mod.rs::all() exactly (same
// order, same names) — "dot" colors are each theme's own `accent` value
// (docs/css/styles.css has the full palette per theme; this file only
// needs enough to build/label the swatches and know which ones have a real
// screenshot). `screenshot: true` themes have a captured PNG in
// docs/assets/screenshots/<id>.png (via scripts/screenshots.sh); the rest
// fall back to the CSS-only #term-fallback mockup so a wrong-theme's
// screenshot is never shown mislabeled as another theme.
const THEMES = [
  { id: "catppuccin-mocha", label: "Catppuccin Mocha", dot: "#89b4fa", screenshot: true },
  { id: "catppuccin-macchiato", label: "Catppuccin Macchiato", dot: "#8aadf4", screenshot: false },
  { id: "catppuccin-frappe", label: "Catppuccin Frappé", dot: "#8caaee", screenshot: false },
  { id: "catppuccin-latte", label: "Catppuccin Latte", dot: "#1e66f5", screenshot: false },
  { id: "tokyo-night-storm", label: "Tokyo Night Storm", dot: "#7aa2f7", screenshot: true },
  { id: "tokyo-night", label: "Tokyo Night", dot: "#7aa2f7", screenshot: false },
  { id: "tokyo-night-moon", label: "Tokyo Night Moon", dot: "#82aaff", screenshot: false },
  { id: "gruvbox-dark", label: "Gruvbox Dark", dot: "#458588", screenshot: true },
  { id: "gruvbox-light", label: "Gruvbox Light", dot: "#458588", screenshot: false },
  { id: "nord", label: "Nord", dot: "#88c0d0", screenshot: true },
  { id: "solarized-dark", label: "Solarized Dark", dot: "#268bd2", screenshot: true },
  { id: "solarized-light", label: "Solarized Light", dot: "#268bd2", screenshot: false },
];

const STORAGE_KEY = "shiki-site-theme";
const DEFAULT_THEME = "gruvbox-dark"; // matches ThemeConfig::default() as of shiki 0.8.1+

function applyTheme(themeId) {
  const theme = THEMES.find((t) => t.id === themeId) || THEMES.find((t) => t.id === DEFAULT_THEME);

  document.documentElement.setAttribute("data-theme", theme.id);

  document.querySelectorAll(".swatch").forEach((el) => {
    el.classList.toggle("active", el.dataset.themeId === theme.id);
  });

  const img = document.getElementById("theme-screenshot");
  const fallback = document.getElementById("term-fallback");
  const title = document.getElementById("theme-screenshot-title");
  const caption = document.getElementById("screenshot-caption");

  if (theme.screenshot) {
    img.src = `assets/screenshots/${theme.id}.png`;
    img.hidden = false;
    fallback.hidden = true;
    caption.textContent = "Real screenshot, captured with this exact theme.";
  } else {
    img.hidden = true;
    fallback.hidden = false;
    caption.textContent =
      "Live CSS mockup (a real screenshot for this palette isn't captured yet) — colors are still the exact values shiki uses.";
  }
  title.textContent = `shiki — theme: ${theme.id}`;

  try {
    localStorage.setItem(STORAGE_KEY, theme.id);
  } catch (e) {
    // localStorage can throw in private-browsing/blocked-storage contexts —
    // theme switching still works for the current page view either way.
  }
}

function buildSwatches() {
  const container = document.getElementById("theme-swatches");
  THEMES.forEach((theme) => {
    const btn = document.createElement("button");
    btn.className = "swatch";
    btn.type = "button";
    btn.dataset.themeId = theme.id;
    btn.innerHTML = `<span class="dot" style="background:${theme.dot}"></span>${theme.label}`;
    btn.addEventListener("click", () => applyTheme(theme.id));
    container.appendChild(btn);
  });
}

function initTheme() {
  let saved = null;
  try {
    saved = localStorage.getItem(STORAGE_KEY);
  } catch (e) {
    // ignore — fall through to the default
  }
  applyTheme(saved || DEFAULT_THEME);
}

// ---------------------------------------------------------------------------
// Changelog: fetched live from CHANGELOG.md on `main` rather than duplicated
// by hand into this page, so it can never go stale relative to the repo.
// Only a small hand-rolled subset of Keep a Changelog's actual format is
// parsed here (## version headers, ### category headers, - bullets, `code`,
// **bold**, [text](url) links) — intentionally not a general markdown
// library, since this only ever needs to render shiki's own CHANGELOG.md.
// ---------------------------------------------------------------------------

const CHANGELOG_URL = "https://raw.githubusercontent.com/sazardev/shiki/main/CHANGELOG.md";
const CHANGELOG_MAX_VERSIONS = 5;

function escapeHtml(str) {
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function renderInline(text) {
  let out = escapeHtml(text);
  out = out.replace(/`([^`]+)`/g, "<code>$1</code>");
  out = out.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
  out = out.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank" rel="noopener">$1</a>');
  return out;
}

function renderChangelog(markdown) {
  const lines = markdown.split("\n");
  let html = "";
  let versionCount = 0;
  let inList = false;
  let skipping = false;
  // Continuation lines of a wrapped bullet are accumulated here as *raw*
  // markdown and only run through `renderInline` once the bullet is known
  // to be complete — CHANGELOG.md hand-wraps long bullets at ~100 columns,
  // sometimes splitting a single `code span` or **bold** run across two
  // physical lines, and matching backtick/asterisk pairs line-by-line
  // (the previous approach) can't see across that line break.
  let pendingBullet = null;

  const closeList = () => {
    flushBullet();
    if (inList) {
      html += "</ul>";
      inList = false;
    }
  };

  const flushBullet = () => {
    if (pendingBullet !== null) {
      html += `<li>${renderInline(pendingBullet)}</li>`;
      pendingBullet = null;
    }
  };

  for (const rawLine of lines) {
    const line = rawLine.trimEnd();

    const versionMatch = line.match(/^##\s+\[([^\]]+)\]\s*(-\s*(.+))?/);
    if (versionMatch) {
      versionCount += 1;
      if (versionCount > CHANGELOG_MAX_VERSIONS) {
        skipping = true;
        continue;
      }
      skipping = false;
      closeList();
      const date = versionMatch[3] ? `<span class="cl-date"> — ${escapeHtml(versionMatch[3])}</span>` : "";
      html += `<h3>${escapeHtml(versionMatch[1])}${date}</h3>`;
      continue;
    }

    if (skipping) continue;

    const categoryMatch = line.match(/^###\s+(.+)/);
    if (categoryMatch) {
      closeList();
      html += `<h4>${escapeHtml(categoryMatch[1])}</h4>`;
      continue;
    }

    const bulletMatch = line.match(/^-\s+(.+)/);
    if (bulletMatch) {
      flushBullet();
      if (!inList) {
        html += "<ul>";
        inList = true;
      }
      pendingBullet = bulletMatch[1];
      continue;
    }

    // A continuation line of a multi-line bullet (indented, no leading `-`)
    // extends the raw text of the bullet still being accumulated, instead
    // of being rendered and appended on its own.
    if (inList && pendingBullet !== null && line.trim().length > 0 && /^\s/.test(rawLine)) {
      pendingBullet += ` ${line.trim()}`;
      continue;
    }

    if (line.trim().length === 0) {
      closeList();
    }
  }
  closeList();
  return html || "<p>No changelog entries found.</p>";
}

async function loadChangelog() {
  const container = document.getElementById("changelog-content");
  try {
    const res = await fetch(CHANGELOG_URL);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const text = await res.text();
    container.innerHTML = renderChangelog(text);
  } catch (err) {
    container.innerHTML = `<p class="changelog-error">Couldn't load the live changelog right now. See it directly on <a href="https://github.com/sazardev/shiki/blob/main/CHANGELOG.md">GitHub</a>.</p>`;
  }
}

// ---------------------------------------------------------------------------
// Live "latest version" — fetched from the GitHub Releases API on every page
// load rather than hardcoded, so a new tagged release shows up here with no
// site redeploy at all (the same reasoning as the live changelog fetch
// above). `.github/workflows/release.yml`'s `update-screenshots` job is what
// keeps the *screenshots* themselves current after each release; this is
// the lightweight text-only counterpart for the version number itself.
// ---------------------------------------------------------------------------

const LATEST_RELEASE_URL = "https://api.github.com/repos/sazardev/shiki/releases/latest";

async function loadLatestVersion() {
  try {
    const res = await fetch(LATEST_RELEASE_URL);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const data = await res.json();
    const tag = data.tag_name; // e.g. "v0.8.1"
    if (!tag) return;

    const downloadBtn = document.getElementById("download-btn");
    if (downloadBtn) downloadBtn.textContent = `Download ${tag}`;

    const pill = document.getElementById("version-pill");
    if (pill) {
      pill.textContent = `latest: ${tag}`;
      pill.hidden = false;
    }
  } catch (err) {
    // Silent failure — the buttons already have sensible static fallback
    // text/links (GitHub's own "latest" redirect), so a failed fetch here
    // (offline, GitHub API rate limit) degrades gracefully with no broken UI.
  }
}

document.addEventListener("DOMContentLoaded", () => {
  buildSwatches();
  initTheme();
  loadChangelog();
  loadLatestVersion();
});
