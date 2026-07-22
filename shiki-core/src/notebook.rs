use std::path::{Path, PathBuf};

use crate::note::{Frontmatter, Note};
use crate::{git, Error, Result};

/// A notebook is a directory with its own git repo, containing `.md` notes.
#[derive(Debug, Clone)]
pub struct Notebook {
    pub name: String,
    pub path: PathBuf,
}

impl Notebook {
    pub fn new(name: impl Into<String>, path: PathBuf) -> Self {
        Self {
            name: name.into(),
            path,
        }
    }

    /// Lists the immediate contents of `relative` (a path within this
    /// notebook; `""` for the notebook root itself): subfolder names and
    /// notes, separately — a notebook can be nested arbitrarily deep, same
    /// as `nb`, and the caller (the Notes panel) walks one level at a time.
    /// Folders are sorted alphabetically; `.git` is never listed as a folder.
    ///
    /// `.md` files that don't parse as a shiki note (no `---` frontmatter —
    /// common in an imported/pre-existing repo, or one from `nb`) still show
    /// up: `Note::from_file` synthesizes metadata for those rather than
    /// failing, so nothing here needs to skip them.
    pub fn list_dir(&self, relative: &Path) -> Result<(Vec<String>, Vec<Note>)> {
        let dir = self.path.join(relative);
        if !dir.exists() {
            return Ok((Vec::new(), Vec::new()));
        }
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        entries.sort();

        let mut folders = Vec::new();
        let mut notes = Vec::new();
        for path in entries {
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n == ".git") {
                    continue;
                }
                if let Some(name) = path.file_name() {
                    folders.push(name.to_string_lossy().to_string());
                }
            } else if path.extension().is_some_and(|ext| ext == "md") {
                notes.push(Note::from_file(&path)?);
            }
        }
        Ok((folders, notes))
    }

    /// Notes at the notebook's root only — the common case (CLI commands,
    /// the daily note check) that doesn't care about subfolders.
    pub fn list_notes(&self) -> Result<Vec<Note>> {
        Ok(self.list_dir(Path::new(""))?.1)
    }

    /// Every note in the notebook, at any folder depth — the pool for a
    /// global (cross-notebook) search, so nested notes are still findable.
    pub fn all_notes_recursive(&self) -> Result<Vec<Note>> {
        let mut out = Vec::new();
        self.collect_notes(Path::new(""), &mut out)?;
        Ok(out)
    }

    fn collect_notes(&self, relative: &Path, out: &mut Vec<Note>) -> Result<()> {
        let (folders, notes) = self.list_dir(relative)?;
        out.extend(notes);
        for folder in folders {
            self.collect_notes(&relative.join(folder), out)?;
        }
        Ok(())
    }

    /// Creates a new note from a title and an initial body, in `relative`
    /// (a path within this notebook; `""` for the notebook root).
    pub fn create_note_in(
        &self,
        relative: &Path,
        title: &str,
        body: impl Into<String>,
    ) -> Result<Note> {
        let dir = self.path.join(relative);
        std::fs::create_dir_all(&dir)?;
        let slug = Note::slugify(title);
        let path = dir.join(format!("{slug}.md"));
        let note = Note::new(path, Frontmatter::new(title, &self.name), body.into());
        note.save()?;
        Ok(note)
    }

    pub fn create_note(&self, title: &str, body: impl Into<String>) -> Result<Note> {
        self.create_note_in(Path::new(""), title, body)
    }

    pub fn note_path(&self, slug: &str) -> PathBuf {
        self.path.join(format!("{slug}.md"))
    }

    /// Deletes the note at its actual path (wherever it lives — root or a
    /// nested folder), not a path reconstructed from a root-relative slug.
    pub fn delete_note_at(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Err(Error::NoteNotFound(path.display().to_string()));
        }
        std::fs::remove_file(path)?;
        Ok(())
    }

    /// Renames the note at `path`, keeping it in the same folder.
    pub fn rename_note_at(&self, path: &Path, new_title: &str) -> Result<Note> {
        let mut note = Note::from_file(path)?;
        let dir = path.parent().unwrap_or(&self.path);
        let new_path = dir.join(format!("{}.md", Note::slugify(new_title)));
        note.frontmatter.title = new_title.to_string();
        note.path = new_path;
        note.save()?;
        if path != note.path {
            std::fs::remove_file(path)?;
        }
        Ok(note)
    }
}

/// Manages the collection of notebooks under the data directory (`~/.local/share/shiki/`).
#[derive(Debug, Clone)]
pub struct NotebookStore {
    pub root: PathBuf,
}

/// Rejects names that would escape `root` when joined as a path component —
/// empty, `.`/`..`, or containing a path separator. Notebook names come
/// straight from user input (the "new notebook" prompt), so without this a
/// name like `..` or `foo/bar` would silently create/delete outside the
/// intended data directory.
fn validate_name(name: &str) -> Result<()> {
    let invalid =
        name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\');
    if invalid {
        return Err(Error::InvalidName(name.to_string()));
    }
    Ok(())
}

impl NotebookStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn list(&self) -> Result<Vec<Notebook>> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut notebooks: Vec<Notebook> = std::fs::read_dir(&self.root)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .map(|p| {
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                Notebook::new(name, p)
            })
            .collect();
        notebooks.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(notebooks)
    }

    /// Every note across every notebook (any folder depth), paired with the
    /// notebook it lives in — the pool for a global (cross-notebook) search.
    pub fn all_notes(&self) -> Result<Vec<(Notebook, Note)>> {
        let mut all = Vec::new();
        for notebook in self.list()? {
            for note in notebook.all_notes_recursive()? {
                all.push((notebook.clone(), note));
            }
        }
        Ok(all)
    }

    pub fn get(&self, name: &str) -> Result<Notebook> {
        validate_name(name)?;
        let path = self.root.join(name);
        if !path.is_dir() {
            return Err(Error::NotebookNotFound(name.to_string()));
        }
        Ok(Notebook::new(name, path))
    }

    /// Creates a new notebook with its own git repo.
    pub fn create(&self, name: &str) -> Result<Notebook> {
        validate_name(name)?;
        let path = self.root.join(name);
        if path.exists() {
            return Err(Error::NotebookExists(name.to_string()));
        }
        std::fs::create_dir_all(&path)?;
        git::init_repo(&path)?;
        Ok(Notebook::new(name, path))
    }

    pub fn rename(&self, old_name: &str, new_name: &str) -> Result<Notebook> {
        validate_name(old_name)?;
        validate_name(new_name)?;
        let old_path = self.root.join(old_name);
        if !old_path.is_dir() {
            return Err(Error::NotebookNotFound(old_name.to_string()));
        }
        let new_path = self.root.join(new_name);
        if new_path.exists() {
            return Err(Error::NotebookExists(new_name.to_string()));
        }
        std::fs::rename(&old_path, &new_path)?;
        Ok(Notebook::new(new_name, new_path))
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        validate_name(name)?;
        let path = self.root.join(name);
        if !path.is_dir() {
            return Err(Error::NotebookNotFound(name.to_string()));
        }
        std::fs::remove_dir_all(path)?;
        Ok(())
    }
}

pub fn ensure_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    Ok(())
}
