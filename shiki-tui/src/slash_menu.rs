use shiki_config::Config;

/// One entry in the inline editor's `/`-menu — either one of `builtins()`
/// or a `[[snippets]]` entry from `config.toml` (`Config::snippets`, see
/// `shiki_config::config::SnippetConfig` for the on-disk shape).
#[derive(Debug, Clone)]
pub struct SlashCommand {
    pub trigger: String,
    pub label: String,
    /// `{{title}}`/`{{date}}` are substituted the same way note templates
    /// are (`shiki_core::Template::render` — `App::apply_slash_command`
    /// reuses it directly). A literal `{{cursor}}` marks where the cursor
    /// should land after insertion; it's resolved separately and never
    /// ends up in the actually-inserted text. Omit it to leave the cursor
    /// at the end of the snippet, which is already correct for anything
    /// that's just one line with nothing to fill in (`h1`, `divider`).
    pub body: String,
}

fn builtin(trigger: &str, label: &str, body: &str) -> SlashCommand {
    SlashCommand {
        trigger: trigger.to_string(),
        label: label.to_string(),
        body: body.to_string(),
    }
}

/// The commands every install starts with — deliberately not persisted to
/// `config.toml` (unlike templates, which `ensure_defaults` writes to
/// disk): there's nothing to customize by hand-editing a file here unless
/// a user actually wants to, so nothing is written until they add their
/// own `[[snippets]]` entry.
pub fn builtins() -> Vec<SlashCommand> {
    vec![
        builtin("h1", "Heading 1", "# {{cursor}}"),
        builtin("h2", "Heading 2", "## {{cursor}}"),
        builtin("h3", "Heading 3", "### {{cursor}}"),
        builtin("code", "Code block", "```\n{{cursor}}\n```"),
        builtin("math", "Math block", "$$\n{{cursor}}\n$$"),
        builtin(
            "table",
            "Table",
            "| Column | Column |\n| --- | --- |\n| {{cursor}} |  |\n",
        ),
        builtin("check", "Checklist item", "- [ ] {{cursor}}"),
        builtin("quote", "Quote", "> {{cursor}}"),
        builtin("divider", "Divider", "---\n"),
        builtin("date", "Today's date", "{{date}}"),
        builtin("tags", "Tags line", "Tags: {{cursor}}"),
        builtin(
            "frontmatter",
            "YAML frontmatter block",
            "---\ntitle: {{title}}\ndate: {{date}}\ntags: []\n---\n{{cursor}}",
        ),
    ]
}

/// The full `/`-menu list: built-ins, with any `config.toml`
/// `[snippets.<trigger>]` entry of the same trigger (case-insensitive)
/// overriding it in place — so a user can redefine `code` or `h1` just as
/// easily as add a brand new command — otherwise appended after the
/// built-ins.
pub fn all_commands(config: &Config) -> Vec<SlashCommand> {
    let mut commands = builtins();
    for (trigger, custom) in &config.snippets {
        let entry = SlashCommand {
            trigger: trigger.clone(),
            label: custom.label.clone().unwrap_or_else(|| trigger.clone()),
            body: custom.body.clone(),
        };
        match commands
            .iter_mut()
            .find(|c| c.trigger.eq_ignore_ascii_case(&entry.trigger))
        {
            Some(existing) => *existing = entry,
            None => commands.push(entry),
        }
    }
    commands
}

#[cfg(test)]
mod tests {
    use super::*;
    use shiki_config::config::SnippetConfig;

    #[test]
    fn custom_snippet_is_appended() {
        let mut config = Config::default();
        config.snippets.insert(
            "note".to_string(),
            SnippetConfig {
                label: Some("Callout".to_string()),
                body: "> {{cursor}}".to_string(),
            },
        );
        let commands = all_commands(&config);
        assert_eq!(commands.len(), builtins().len() + 1);
        assert!(commands.iter().any(|c| c.trigger == "note"));
    }

    #[test]
    fn custom_snippet_overrides_a_builtin_case_insensitively() {
        let mut config = Config::default();
        config.snippets.insert(
            "H1".to_string(),
            SnippetConfig {
                label: None,
                body: "# custom {{cursor}}".to_string(),
            },
        );
        let commands = all_commands(&config);
        assert_eq!(commands.len(), builtins().len());
        let h1 = commands.iter().find(|c| c.trigger == "H1").unwrap();
        assert_eq!(h1.body, "# custom {{cursor}}");
        assert_eq!(h1.label, "H1");
    }
}
