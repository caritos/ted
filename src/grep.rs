//! "Find in files": a plain-text search over a list of project files. Pure
//! apart from reading the files; no terminal code.

use std::path::Path;

/// Most hits collected; the search stops there and says so.
pub const MAX_HITS: usize = 2_000;

/// Files bigger than this are not searched (generated or data files).
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// Longest line text kept for a hit.
const MAX_TEXT_CHARS: usize = 300;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Hit {
    /// Relative to the root, as listed.
    pub path: String,
    /// 1-based.
    pub line: usize,
    /// The line without its indent and line break, cut at `MAX_TEXT_CHARS`.
    pub text: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Found {
    pub hits: Vec<Hit>,
    /// The search stopped at `MAX_HITS`.
    pub capped: bool,
    /// Files that held at least one hit.
    pub files_with_hits: usize,
}

/// Lines of the listed files (paths relative to `root`) that contain `query`,
/// in list order. Smart case like the Find prompt: an all-lowercase query
/// ignores case, any capital makes it exact. Files that cannot be read, are
/// not UTF-8, hold a NUL byte or exceed `MAX_FILE_BYTES` are skipped.
pub fn search(root: &Path, files: &[String], query: &str) -> Found {
    let mut found = Found::default();
    if query.is_empty() {
        return found;
    }
    let exact = query.chars().any(char::is_uppercase);
    let needle = if exact { query.to_string() } else { query.to_lowercase() };
    for rel in files {
        let path = root.join(rel);
        match std::fs::metadata(&path) {
            Ok(meta) if meta.is_file() && meta.len() <= MAX_FILE_BYTES => {}
            _ => continue,
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        if bytes.contains(&0) {
            continue;
        }
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        let before = found.hits.len();
        for (i, line) in text.lines().enumerate() {
            let hit = if exact { line.contains(&needle) } else { line.to_lowercase().contains(&needle) };
            if !hit {
                continue;
            }
            if found.hits.len() >= MAX_HITS {
                found.capped = true;
                break;
            }
            let text = line.trim().chars().take(MAX_TEXT_CHARS).collect();
            found.hits.push(Hit { path: rel.clone(), line: i + 1, text });
        }
        if found.hits.len() > before {
            found.files_with_hits += 1;
        }
        if found.capped {
            break;
        }
    }
    found
}

#[cfg(test)]
mod tests;
