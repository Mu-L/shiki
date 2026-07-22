use anyhow::Result;
use shiki_core::NotebookStore;

use super::find_note;

pub fn run(store: &NotebookStore, notebook: &str, note: &str) -> Result<()> {
    let note = find_note(store, notebook, note)?;
    println!("# {}", note.frontmatter.title);
    println!("date: {}", note.frontmatter.date);
    if !note.frontmatter.tags.is_empty() {
        println!("tags: {}", note.frontmatter.tags.join(", "));
    }
    println!();
    println!("{}", note.body);
    Ok(())
}
