use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::Result;

/// YAML frontmatter at the top of every note.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frontmatter {
    pub title: String,
    pub date: NaiveDate,
    #[serde(default)]
    pub tags: Vec<String>,
    pub notebook: String,
    #[serde(default)]
    pub links: Vec<String>,
    #[serde(default)]
    pub template: Option<String>,
}

impl Frontmatter {
    pub fn new(title: impl Into<String>, notebook: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            date: chrono::Local::now().date_naive(),
            tags: Vec::new(),
            notebook: notebook.into(),
            links: Vec::new(),
            template: None,
        }
    }
}

/// A note: path on disk, parsed frontmatter, and markdown body.
#[derive(Debug, Clone)]
pub struct Note {
    pub path: PathBuf,
    pub frontmatter: Frontmatter,
    pub body: String,
}

impl Note {
    pub fn new(path: PathBuf, frontmatter: Frontmatter, body: String) -> Self {
        Self {
            path,
            frontmatter,
            body,
        }
    }

    /// Slug derived from the title: lowercase, spaces -> dashes, no special characters.
    pub fn slugify(title: &str) -> String {
        let mut slug = String::with_capacity(title.len());
        let mut last_was_dash = false;
        for c in title.trim().chars() {
            if c.is_alphanumeric() {
                slug.push(c.to_ascii_lowercase());
                last_was_dash = false;
            } else if !last_was_dash {
                slug.push('-');
                last_was_dash = true;
            }
        }
        slug.trim_matches('-').to_string()
    }

    pub fn file_stem(&self) -> String {
        self.path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    /// Parses a `.md` file. Notes written by shiki have YAML frontmatter
    /// delimited by `---`; anything else (a plain markdown file dropped in
    /// from elsewhere — `nb`, an existing repo, a manual export) is still a
    /// valid note, just without that metadata. Rather than rejecting those,
    /// this synthesizes a title (first `# heading`, else the filename), a
    /// date (the file's mtime), and treats the whole file as the body — see
    /// `synthesize_frontmatter` and `from_file_in_notebook` for the
    /// notebook-aware variant. The only real failure mode left is I/O.
    pub fn from_file(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let (frontmatter, body) = Self::split(path, &contents, None);
        Ok(Self {
            path: path.to_path_buf(),
            frontmatter,
            body,
        })
    }

    /// Like `from_file`, but passes `notebook_name` through to
    /// `synthesize_frontmatter` so the `frontmatter.notebook` field is
    /// correct even for notes nested several folders deep inside the
    /// notebook — the old `synthesize_frontmatter` read the notebook name
    /// from `path.parent().file_name()`, which would pick up an
    /// intermediate folder name instead of the notebook itself.
    pub fn from_file_in_notebook(path: &Path, notebook_name: &str) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let (frontmatter, body) = Self::split(path, &contents, Some(notebook_name));
        Ok(Self {
            path: path.to_path_buf(),
            frontmatter,
            body,
        })
    }

    fn split(path: &Path, contents: &str, notebook: Option<&str>) -> (Frontmatter, String) {
        Self::try_parse_frontmatter(contents).unwrap_or_else(|| {
            (
                Self::synthesize_frontmatter(path, contents, notebook),
                contents.to_string(),
            )
        })
    }

    fn try_parse_frontmatter(contents: &str) -> Option<(Frontmatter, String)> {
        let rest = contents.strip_prefix("---\n")?;
        let end = rest.find("\n---")?;
        let yaml = &rest[..end];
        let body = rest[end + 4..].trim_start_matches('\n').to_string();
        let frontmatter: Frontmatter = serde_yaml::from_str(yaml).ok()?;
        Some((frontmatter, body))
    }

    /// Best-effort metadata for a note that arrived with no frontmatter of
    /// its own. When `notebook` is `Some`, it's used as the `notebook` field
    /// directly; when `None`, falls back to `path.parent().file_name()` for
    /// backward compatibility with callers that don't know the notebook.
    fn synthesize_frontmatter(path: &Path, contents: &str, notebook: Option<&str>) -> Frontmatter {
        let title = contents
            .lines()
            .find_map(|line| line.strip_prefix("# "))
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| Self::title_from_filename(path));
        let notebook = match notebook {
            Some(name) => name.to_string(),
            None => path
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
        };
        let date = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|d| chrono::DateTime::from_timestamp(d.as_secs() as i64, 0))
            .map(|dt| dt.date_naive())
            .unwrap_or_else(|| chrono::Local::now().date_naive());
        Frontmatter {
            title,
            date,
            tags: Vec::new(),
            notebook,
            links: Vec::new(),
            template: None,
        }
    }

    fn title_from_filename(path: &Path) -> String {
        path.file_stem()
            .map(|s| s.to_string_lossy().replace(['-', '_'], " "))
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "Untitled".to_string())
    }

    /// Serializes the full note (frontmatter + body) to the on-disk file format.
    pub fn to_file_contents(&self) -> Result<String> {
        let yaml = serde_yaml::to_string(&self.frontmatter)?;
        Ok(format!("---\n{yaml}---\n\n{}", self.body))
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.path, self.to_file_contents()?)?;
        Ok(())
    }
}
