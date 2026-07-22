use std::collections::HashMap;
use std::path::Path;

use crate::{Error, Result};

/// A note template: text with `{{key}}` placeholders.
#[derive(Debug, Clone)]
pub struct Template {
    pub name: String,
    pub contents: String,
}

impl Template {
    pub fn load(templates_dir: &Path, name: &str) -> Result<Self> {
        let path = templates_dir.join(format!("{name}.md"));
        if !path.exists() {
            return Err(Error::TemplateNotFound(name.to_string()));
        }
        let contents = std::fs::read_to_string(path)?;
        Ok(Self {
            name: name.to_string(),
            contents,
        })
    }

    /// Substitutes `{{key}}` placeholders with the given values.
    pub fn render(&self, vars: &HashMap<&str, String>) -> String {
        let mut out = self.contents.clone();
        for (key, value) in vars {
            out = out.replace(&format!("{{{{{key}}}}}"), value);
        }
        out
    }
}

pub const DEFAULT_TEMPLATE: &str = "# {{title}}\n\n";
pub const DAILY_TEMPLATE: &str = "# {{date}}\n\n## Tasks\n\n- [ ] \n\n## Notes\n\n";
pub const MEETING_TEMPLATE: &str =
    "# {{title}}\n\nDate: {{date}}\n\n## Attendees\n\n## Agenda\n\n## Notes\n\n## Action Items\n\n";

/// Creates the default templates in `templates_dir` if they don't exist yet.
pub fn ensure_defaults(templates_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(templates_dir)?;
    let defaults = [
        ("default.md", DEFAULT_TEMPLATE),
        ("daily.md", DAILY_TEMPLATE),
        ("meeting.md", MEETING_TEMPLATE),
    ];
    for (filename, contents) in defaults {
        let path = templates_dir.join(filename);
        if !path.exists() {
            std::fs::write(path, contents)?;
        }
    }
    Ok(())
}
