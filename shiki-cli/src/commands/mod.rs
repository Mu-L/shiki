pub mod config;
pub mod daily;
pub mod doctor;
pub mod edit;
pub mod list;
pub mod new;
pub mod notebook;
pub mod search;
pub mod show;
pub mod sync;
pub mod theme;

use anyhow::{Context, Result};
use shiki_core::{Note, NotebookStore};

/// Resolves a note by slug or by (case-insensitive) title match within a notebook.
pub fn find_note(store: &NotebookStore, notebook: &str, needle: &str) -> Result<Note> {
    let nb = store
        .get(notebook)
        .with_context(|| format!("notebook '{notebook}' not found"))?;
    let notes = nb.all_notes_recursive()?;
    let slug = shiki_core::note::Note::slugify(needle);
    notes
        .into_iter()
        .find(|n| n.file_stem() == slug || n.frontmatter.title.eq_ignore_ascii_case(needle))
        .with_context(|| format!("note '{needle}' not found in '{notebook}'"))
}

/// Opens `path` with the configured external editor, waiting for it to finish.
pub fn open_in_editor(editor: &str, path: &std::path::Path) -> Result<()> {
    let status = shiki_core::editor::command_for(editor, path)
        .status()
        .with_context(|| format!("could not run editor '{editor}'"))?;
    if !status.success() {
        anyhow::bail!("'{editor}' exited with an error");
    }
    Ok(())
}
