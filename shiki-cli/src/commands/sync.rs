use anyhow::{Context, Result};
use shiki_config::config::GitConfig;
use shiki_core::{git, NotebookStore};

pub fn run(store: &NotebookStore, notebook: &str, git_config: &GitConfig) -> Result<()> {
    let nb = store
        .get(notebook)
        .with_context(|| format!("notebook '{notebook}' not found"))?;
    let message = format!("{}sync", git_config.commit_prefix);
    let committed = git::commit_all(&nb.path, &message)?;
    if committed {
        println!("commit created in '{notebook}'");
    } else {
        println!("'{notebook}' has no changes");
    }
    if git_config.auto_push {
        git::push(&nb.path, &git_config.remote, &git_config.branch)?;
        println!("pushed to {}/{}", git_config.remote, git_config.branch);
    }
    Ok(())
}
