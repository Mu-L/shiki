use anyhow::{Context, Result};
use shiki_core::NotebookStore;

use super::open_in_editor;

pub fn run(store: &NotebookStore, notebook: &str, title: &str, editor: &str) -> Result<()> {
    let nb = match store.get(notebook) {
        Ok(nb) => nb,
        Err(_) => store
            .create(notebook)
            .with_context(|| format!("could not create notebook '{notebook}'"))?,
    };
    let note = nb.create_note(title, "")?;
    println!("created: {}", note.path.display());
    open_in_editor(editor, &note.path)
}
