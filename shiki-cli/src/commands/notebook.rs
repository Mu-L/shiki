use anyhow::Result;
use shiki_core::NotebookStore;

pub fn create(store: &NotebookStore, name: &str) -> Result<()> {
    let nb = store.create(name)?;
    println!("notebook created: {}", nb.path.display());
    Ok(())
}

pub fn list(store: &NotebookStore) -> Result<()> {
    let notebooks = store.list()?;
    if notebooks.is_empty() {
        println!("(no notebooks)");
        return Ok(());
    }
    for nb in notebooks {
        let count = nb.list_notes().map(|n| n.len()).unwrap_or(0);
        println!("{}  ({count} notes)", nb.name);
    }
    Ok(())
}

pub fn rename(store: &NotebookStore, old: &str, new: &str) -> Result<()> {
    let nb = store.rename(old, new)?;
    println!("renamed to: {}", nb.path.display());
    Ok(())
}
