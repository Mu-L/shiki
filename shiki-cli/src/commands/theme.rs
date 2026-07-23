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
    // Only reset overrides when actually switching to a different base
    // theme — re-running `set` on the theme that's already active used to
    // silently wipe any hand-written custom colors for no reason.
    if config.theme.name != name {
        config.theme.overrides = Default::default();
    }
    config.theme.name = name.to_string();
    config.save(&Config::default_path()?)?;
    println!("theme set to '{name}'");
    Ok(())
}

/// Scaffolds every one of the 19 color slots into config.toml's
/// `[theme.overrides]`, copied from a real theme's values — a genuine
/// starting point to edit slot-by-slot, rather than hand-writing hex codes
/// from scratch with no example to copy from. `from` defaults to whichever
/// theme is currently active if not given.
pub fn create(config: &mut Config, from: Option<&str>) -> Result<()> {
    let base_name = from.unwrap_or(&config.theme.name).to_string();
    let base = shiki_config::themes::by_name(&base_name).ok_or_else(|| {
        anyhow::anyhow!(
            "unknown theme '{base_name}' — run `shiki theme list` to see available themes"
        )
    })?;
    // The base theme becomes `theme.name` too, not just the source for the
    // override values — every slot is about to be explicitly overridden
    // with `base`'s own colors, so leaving `theme.name` pointing at
    // whatever was active before would make the printed "falls back to"
    // guidance below wrong the moment someone removes a key.
    config.theme.name = base_name.clone();
    config.theme.overrides = shiki_config::config::ThemeOverrides::from_theme(&base);
    let path = Config::default_path()?;
    config.save(&path)?;
    println!(
        "scaffolded all 19 color slots from '{base_name}' into {}'s [theme.overrides] — edit any \
         value there; removing a key falls back to '{base_name}' for that slot",
        path.display()
    );
    Ok(())
}
