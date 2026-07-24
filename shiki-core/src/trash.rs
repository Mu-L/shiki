//! Delete-with-undo: a note or folder removed from a notebook is moved into
//! a trash directory instead of being permanently deleted, so a single
//! "undo" can put it right back. Not a full undo/redo history — only the
//! most recent delete is restorable (the caller, `shiki-tui`'s `App`, keeps
//! at most one delete operation's worth of trash entries in memory); older
//! trashed items simply stay on disk, unreachable from the undo keybinding
//! but not actually gone.

use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// Moves `source` (a note file or a whole folder, with everything inside
/// it) into `trash_root`, naming it `{unique_suffix}-{original file name}`
/// so a batch delete's same-named items across different folders can't
/// collide with each other in the trash. Returns the path it now lives at,
/// for `restore` to move back later.
pub fn move_to_trash(source: &Path, trash_root: &Path, unique_suffix: &str) -> Result<PathBuf> {
    std::fs::create_dir_all(trash_root)?;
    let name = source
        .file_name()
        .ok_or_else(|| Error::NoteNotFound(source.display().to_string()))?;
    let dest = trash_root.join(format!("{unique_suffix}-{}", name.to_string_lossy()));
    std::fs::rename(source, &dest)?;
    Ok(dest)
}

/// Moves a previously-trashed item back to `original_path`, recreating any
/// parent directories that no longer exist (e.g. the folder it used to live
/// in was itself deleted or renamed in the meantime).
pub fn restore(trash_path: &Path, original_path: &Path) -> Result<()> {
    if let Some(parent) = original_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(trash_path, original_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_to_trash_then_restore_round_trips_a_file() {
        let tmp = tempfile::tempdir().unwrap();
        let original = tmp.path().join("notebook/note.md");
        std::fs::create_dir_all(original.parent().unwrap()).unwrap();
        std::fs::write(&original, "body").unwrap();
        let trash_root = tmp.path().join("trash/notebook");

        let trashed = move_to_trash(&original, &trash_root, "1700000000-0").unwrap();

        assert!(!original.exists());
        assert!(trashed.exists());
        assert_eq!(std::fs::read_to_string(&trashed).unwrap(), "body");

        restore(&trashed, &original).unwrap();

        assert!(original.exists());
        assert!(!trashed.exists());
        assert_eq!(std::fs::read_to_string(&original).unwrap(), "body");
    }

    #[test]
    fn move_to_trash_moves_a_whole_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let original = tmp.path().join("notebook/scratch");
        std::fs::create_dir_all(&original).unwrap();
        std::fs::write(original.join("note.md"), "x").unwrap();
        let trash_root = tmp.path().join("trash/notebook");

        let trashed = move_to_trash(&original, &trash_root, "1700000000-0").unwrap();

        assert!(!original.exists());
        assert!(trashed.join("note.md").exists());
    }

    #[test]
    fn restore_recreates_missing_parent_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let original = tmp.path().join("notebook/folder-that-got-removed/note.md");
        let trash_root = tmp.path().join("trash/notebook");
        std::fs::create_dir_all(&trash_root).unwrap();
        let trashed = trash_root.join("1700000000-0-note.md");
        std::fs::write(&trashed, "body").unwrap();

        restore(&trashed, &original).unwrap();

        assert_eq!(std::fs::read_to_string(&original).unwrap(), "body");
    }

    #[test]
    fn distinct_unique_suffixes_avoid_collisions_for_same_named_items() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("nb/a/note.md");
        let b = tmp.path().join("nb/b/note.md");
        std::fs::create_dir_all(a.parent().unwrap()).unwrap();
        std::fs::create_dir_all(b.parent().unwrap()).unwrap();
        std::fs::write(&a, "a").unwrap();
        std::fs::write(&b, "b").unwrap();
        let trash_root = tmp.path().join("trash/nb");

        let trashed_a = move_to_trash(&a, &trash_root, "1700000000-0").unwrap();
        let trashed_b = move_to_trash(&b, &trash_root, "1700000000-1").unwrap();

        assert_ne!(trashed_a, trashed_b);
        assert_eq!(std::fs::read_to_string(&trashed_a).unwrap(), "a");
        assert_eq!(std::fs::read_to_string(&trashed_b).unwrap(), "b");
    }
}
