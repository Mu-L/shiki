use anyhow::{Context, Result};
use shiki_core::NotebookStore;

pub fn run(store: &NotebookStore, notebook: &str) -> Result<()> {
    let nb = store
        .get(notebook)
        .with_context(|| format!("notebook '{notebook}' not found"))?;
    let notes = nb.list_notes()?;
    if notes.is_empty() {
        println!("({notebook} is empty)");
        return Ok(());
    }
    for note in notes {
        let tags = if note.frontmatter.tags.is_empty() {
            String::new()
        } else {
            format!("  [{}]", note.frontmatter.tags.join(", "))
        };
        println!(
            "{}  {}{tags}",
            note.frontmatter.date, note.frontmatter.title
        );
    }
    Ok(())
}
