use anyhow::Result;
use shiki_config::Config;

pub fn run() -> Result<()> {
    let path = Config::default_path()?;
    println!("{}", path.display());
    if path.to_string_lossy().contains(' ') {
        eprintln!(
            "note: this path contains spaces — quote it in shell commands, e.g. cat \"{}\"",
            path.display()
        );
    }
    Ok(())
}
