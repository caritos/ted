use std::fmt;

const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Modified,
    Added,
    Deleted,
    Untracked,
}

impl ChangeKind {
    pub fn letter(&self) -> char {
        match self {
            ChangeKind::Modified => 'M',
            ChangeKind::Added => 'A',
            ChangeKind::Deleted => 'D',
            ChangeKind::Untracked => '?',
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub kind: ChangeKind,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Baseline {
    Empty,
    Commit(String),
}

impl Baseline {
    pub fn rev(&self) -> &str {
        match self {
            Baseline::Empty => EMPTY_TREE,
            Baseline::Commit(hash) => hash,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub hash: String,
    pub short: String,
    pub subject: String,
    pub unix_time: i64,
    pub refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitError {
    NotARepo,
    Failed(String),
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitError::NotARepo => write!(f, "not a git repository"),
            GitError::Failed(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for GitError {}

/// Parse `git diff --name-status -z` output (`M\0path\0A\0path\0`).
pub fn parse_name_status_z(out: &str) -> Vec<Change> {
    let mut fields = out.split('\0');
    let mut changes = Vec::new();
    while let (Some(status), Some(path)) = (fields.next(), fields.next()) {
        if status.is_empty() || path.is_empty() {
            break;
        }
        let kind = match status.chars().next() {
            Some('A') => ChangeKind::Added,
            Some('D') => ChangeKind::Deleted,
            _ => ChangeKind::Modified,
        };
        changes.push(Change { kind, path: path.to_string() });
    }
    changes
}

/// Parse `git ls-files --others --exclude-standard -z` output.
pub fn parse_untracked_z(out: &str) -> Vec<Change> {
    out.split('\0')
        .filter(|p| !p.is_empty())
        .map(|p| Change { kind: ChangeKind::Untracked, path: p.to_string() })
        .collect()
}

/// Parse log lines of `%H\t%h\t%ct\t%D\t%s`, skipping malformed lines.
pub fn parse_log(out: &str) -> Vec<Commit> {
    out.lines().filter_map(parse_log_line).collect()
}

fn parse_log_line(line: &str) -> Option<Commit> {
    let mut parts = line.splitn(5, '\t');
    let hash = parts.next()?;
    let short = parts.next()?;
    let unix_time = parts.next()?.parse::<i64>().ok()?;
    let refs = parts.next()?;
    let subject = parts.next()?;
    if hash.is_empty() {
        return None;
    }
    let refs = refs
        .split(", ")
        .map(|r| {
            let r = r.trim();
            let r = r.strip_prefix("HEAD -> ").unwrap_or(r);
            r.strip_prefix("tag: ").unwrap_or(r)
        })
        .filter(|r| !r.is_empty() && *r != "HEAD")
        .map(str::to_string)
        .collect();
    Some(Commit {
        hash: hash.to_string(),
        short: short.to_string(),
        subject: subject.to_string(),
        unix_time,
        refs,
    })
}

pub fn format_age(secs: u64) -> String {
    match secs {
        0..=59 => format!("{secs}s"),
        60..=3599 => format!("{}m", secs / 60),
        3600..=86399 => format!("{}h", secs / 3600),
        _ => format!("{}d", secs / 86400),
    }
}

#[cfg(test)]
mod tests;
