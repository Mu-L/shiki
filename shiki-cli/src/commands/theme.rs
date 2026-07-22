use anyhow::Result;
use shiki_config::Config;

pub fn list(config: &Config) -> Result<()> {
    for theme in shiki_config::themes::all() {
        let marker = if theme.name == config.theme.name {
            "*"
        } else {
            " "
        };
        println!("{marker} {}", theme.name);
    }
    Ok(())
}

pub fn set(config: &mut Config, name: &str) -> Result<()> {
    if shiki_config::themes::by_name(name).is_none() {
        anyhow::bail!("unknown theme '{name}' — run `shiki theme list` to see available themes");
    }
    config.theme.name = name.to_string();
    config.theme.overrides = Default::default();
    config.save(&Config::default_path()?)?;
    println!("theme set to '{name}'");
    Ok(())
}
