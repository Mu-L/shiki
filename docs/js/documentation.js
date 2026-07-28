// Search / command palette for documentation.html only (this file isn't
// loaded by any other page). Indexes the page's own DOM at load time —
// every section heading and every keybinding/config-table row — rather
// than a hand-maintained index that could drift from the actual content
// above it. Pure client-side substring filtering; no build step, no
// search service, consistent with the rest of this site.

function buildDocSearchIndex() {
  const entries = [];

  document.querySelectorAll(".doc-page section[id]").forEach((section) => {
    const heading = section.querySelector("h2");
    if (!heading) return;
    const sectionLabel = heading.textContent.trim();

    entries.push({
      type: "Section",
      label: sectionLabel,
      detail: "",
      target: heading,
      haystack: sectionLabel.toLowerCase(),
    });

    section.querySelectorAll("h3[id], h3").forEach((h3) => {
      const label = h3.textContent.trim();
      entries.push({
        type: sectionLabel,
        label,
        detail: "",
        target: h3,
        haystack: `${sectionLabel} ${label}`.toLowerCase(),
      });
    });

    section.querySelectorAll(".ref-table tbody tr").forEach((row) => {
      const cells = row.querySelectorAll("td");
      if (cells.length < 2) return;
      const key = cells[0].textContent.trim();
      // The description is whichever cell is last (2 columns for plain
      // keybinding tables, 3 for the config tables that also show a
      // Default column) — always the descriptive one worth matching on.
      const detail = cells[cells.length - 1].textContent.trim();
      entries.push({
        type: "Keybinding/option",
        label: key,
        detail: detail.length > 140 ? `${detail.slice(0, 140)}…` : detail,
        target: row,
        haystack: `${key} ${detail}`.toLowerCase(),
      });
    });

    section.querySelectorAll(".doc-callout").forEach((callout) => {
      const title = callout.querySelector("strong");
      if (!title) return;
      const label = title.textContent.trim();
      entries.push({
        type: "How to",
        label,
        detail: "",
        target: callout,
        haystack: `${sectionLabel} ${label}`.toLowerCase(),
      });
    });
  });

  return entries;
}

function flashTarget(el) {
  const row = el.closest("tr") || el;
  row.classList.add("doc-search-highlight");
  row.scrollIntoView({ behavior: "smooth", block: "center" });
  setTimeout(() => row.classList.remove("doc-search-highlight"), 1600);
}

function initDocSearch() {
  const input = document.getElementById("doc-search-input");
  const resultsBox = document.getElementById("doc-search-results");
  if (!input || !resultsBox) return; // only present on documentation.html

  const index = buildDocSearchIndex();
  let activeIndex = -1;
  let currentMatches = [];

  function render(matches) {
    currentMatches = matches;
    activeIndex = matches.length > 0 ? 0 : -1;
    if (matches.length === 0) {
      resultsBox.innerHTML = '<p class="doc-search-empty">No matches.</p>';
      resultsBox.hidden = false;
      return;
    }
    resultsBox.innerHTML = matches
      .slice(0, 20)
      .map(
        (m, i) => `
        <a href="#" class="doc-search-result${i === 0 ? " active" : ""}" data-index="${i}">
          <div class="result-section">${escapeHtml(m.type)}</div>
          <div class="result-text"><code>${escapeHtml(m.label)}</code>${m.detail ? " — " + escapeHtml(m.detail) : ""}</div>
        </a>`
      )
      .join("");
    resultsBox.hidden = false;
  }

  function updateActive(newIndex) {
    const items = resultsBox.querySelectorAll(".doc-search-result");
    if (items.length === 0) return;
    activeIndex = (newIndex + items.length) % items.length;
    items.forEach((item, i) => item.classList.toggle("active", i === activeIndex));
    items[activeIndex].scrollIntoView({ block: "nearest" });
  }

  function selectActive() {
    const match = currentMatches[activeIndex];
    if (!match) return;
    // Hide the dropdown and blur *before* scrolling, not after — closing
    // the dropdown shrinks the page above the target, and doing that
    // mid-flight during a smooth scrollIntoView cancelled the animation
    // outright (confirmed live: the scroll consistently stalled at the
    // same ~60px, matching the dropdown's own height, instead of reaching
    // the target). Nothing about the target's position is still moving by
    // the time flashTarget runs this way.
    resultsBox.hidden = true;
    input.blur();
    flashTarget(match.target);
  }

  input.addEventListener("input", () => {
    const query = input.value.trim().toLowerCase();
    if (!query) {
      resultsBox.hidden = true;
      return;
    }
    const matches = index.filter((e) => e.haystack.includes(query));
    render(matches);
  });

  input.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      updateActive(activeIndex + 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      updateActive(activeIndex - 1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      selectActive();
    } else if (e.key === "Escape") {
      resultsBox.hidden = true;
      input.blur();
    }
  });

  resultsBox.addEventListener("click", (e) => {
    const item = e.target.closest(".doc-search-result");
    if (!item) return;
    e.preventDefault();
    activeIndex = Number(item.dataset.index);
    selectActive();
  });

  document.addEventListener("click", (e) => {
    if (!e.target.closest(".doc-search")) resultsBox.hidden = true;
  });

  // Global "/" focuses search, same convention GitHub/most docs sites use
  // — guarded so it doesn't hijack "/" while already typing somewhere else
  // (a config example's own text, a keybinding row that happens to contain
  // "/" as a literal key, etc.).
  document.addEventListener("keydown", (e) => {
    if (e.key !== "/") return;
    const active = document.activeElement;
    const isTyping =
      active && (active.tagName === "INPUT" || active.tagName === "TEXTAREA" || active.isContentEditable);
    if (isTyping) return;
    e.preventDefault();
    input.focus();
  });
}

document.addEventListener("DOMContentLoaded", initDocSearch);
