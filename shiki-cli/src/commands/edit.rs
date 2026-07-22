use anyhow::Result;
use shiki_core::NotebookStore;

use super::{find_note, open_in_editor};

pub fn run(store: &NotebookStore, notebook: &str, note: &str, editor: &str) -> Result<()> {
    let note = find_note(store, notebook, note)?;
    open_in_editor(editor, &note.path)
}
