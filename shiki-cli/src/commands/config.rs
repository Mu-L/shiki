use anyhow::Result;
use shiki_config::Config;

pub fn run() -> Result<()> {
    let path = Config::default_path()?;
    println!("{}", path.display());
    Ok(())
}
