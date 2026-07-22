use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::note::Note;

/// Index of tags -> notes containing them, for the tag-filtering panel.
#[derive(Debug, Default, Clone)]
pub struct TagIndex {
    index: BTreeMap<String, Vec<PathBuf>>,
}

impl TagIndex {
    pub fn build(notes: &[Note]) -> Self {
        let mut index: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
        for note in notes {
            for tag in &note.frontmatter.tags {
                index
                    .entry(tag.clone())
                    .or_default()
                    .push(note.path.clone());
            }
        }
        Self { index }
    }

    pub fn tags(&self) -> impl Iterator<Item = &String> {
        self.index.keys()
    }

    pub fn notes_for(&self, tag: &str) -> &[PathBuf] {
        self.index.get(tag).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn len(&self) -> usize {
        self.index.len()
    }

    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }
}
