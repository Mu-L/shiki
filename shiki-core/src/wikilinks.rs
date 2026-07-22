use std::collections::HashMap;
use std::path::PathBuf;

use regex::Regex;
use std::sync::OnceLock;

/// Finds all `[[wikilinks]]` in a note's markdown body.
pub fn extract(body: &str) -> Vec<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\[\[([^\[\]|]+)(?:\|[^\[\]]+)?\]\]").unwrap());
    re.captures_iter(body)
        .map(|c| c[1].trim().to_string())
        .collect()
}

/// Resolves wikilink targets (by slug) to existing note paths within a notebook.
pub fn resolve(
    links: &[String],
    notebook_path: &std::path::Path,
) -> HashMap<String, Option<PathBuf>> {
    links
        .iter()
        .map(|link| {
            let slug = crate::note::Note::slugify(link);
            let path = notebook_path.join(format!("{slug}.md"));
            let resolved = path.exists().then_some(path);
            (link.clone(), resolved)
        })
        .collect()
}
