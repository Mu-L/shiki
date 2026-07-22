use std::collections::HashMap;
use std::path::Path;

use chrono::NaiveDate;

use crate::note::{Frontmatter, Note};
use crate::notebook::Notebook;
use crate::templates::Template;
use crate::Result;

/// File name of the daily note for a given date: `YYYY-MM-DD-daily.md`.
pub fn daily_note_path(notebook: &Notebook, date: NaiveDate) -> std::path::PathBuf {
    notebook
        .path
        .join(format!("{}-daily.md", date.format("%Y-%m-%d")))
}

/// Creates (or opens, if it already exists) today's daily note in the given notebook.
pub fn create_or_open(notebook: &Notebook, date: NaiveDate, templates_dir: &Path) -> Result<Note> {
    let path = daily_note_path(notebook, date);
    if path.exists() {
        return Note::from_file(&path);
    }

    let body = match Template::load(templates_dir, "daily") {
        Ok(template) => {
            let mut vars = HashMap::new();
            vars.insert("date", date.format("%Y-%m-%d").to_string());
            template.render(&vars)
        }
        Err(_) => format!(
            "# {}\n\n## Tasks\n\n- [ ] \n\n## Notes\n\n",
            date.format("%Y-%m-%d")
        ),
    };

    let mut frontmatter =
        Frontmatter::new(format!("{} Daily", date.format("%Y-%m-%d")), &notebook.name);
    frontmatter.date = date;
    frontmatter.template = Some("daily".to_string());

    let note = Note::new(path, frontmatter, body);
    note.save()?;
    Ok(note)
}
