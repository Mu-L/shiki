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
    /// `synthesize_frontmatter`. The only real failure mode left is I/O.
    pub fn from_file(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let (frontmatter, body) = Self::split(path, &contents);
        Ok(Self {
            path: path.to_path_buf(),
            frontmatter,
            body,
        })
    }

    fn split(path: &Path, contents: &str) -> (Frontmatter, String) {
        Self::try_parse_frontmatter(contents).unwrap_or_else(|| {
            (
                Self::synthesize_frontmatter(path, contents),
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
    /// its own. The notebook name is read off the parent directory rather
    /// than passed in, since every caller already has a path inside some
    /// notebook's directory.
    fn synthesize_frontmatter(path: &Path, contents: &str) -> Frontmatter {
        let title = contents
            .lines()
            .find_map(|line| line.strip_prefix("# "))
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| Self::title_from_filename(path));
        let notebook = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
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
