use std::fmt;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;

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

pub const MAX_DIFF_BYTES: usize = 200_000;

pub trait Git {
    /// Current HEAD; `Baseline::Empty` when the repo has no commits.
    fn head(&self) -> Result<Baseline, GitError>;
    fn baseline_exists(&self, base: &Baseline) -> bool;
    /// Tracked changes since `base` plus untracked files, scoped to the
    /// location with paths relative to it. No ordering guarantee.
    fn changes(&self, base: &Baseline) -> Result<Vec<Change>, GitError>;
    /// Unified diff text, capped at `MAX_DIFF_BYTES`.
    fn diff(&self, base: &Baseline, change: &Change) -> Result<String, GitError>;
    fn log(&self, limit: usize) -> Result<Vec<Commit>, GitError>;
    /// Mtime of the working-tree file, `None` if it does not exist.
    fn modified(&self, path: &str) -> Option<SystemTime>;
}

pub struct GitCli {
    dir: PathBuf,
}

impl GitCli {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        GitCli { dir: dir.into() }
    }

    /// Base command for every git call. `GIT_OPTIONAL_LOCKS=0` keeps polling
    /// from refreshing the index or taking `index.lock`, so the viewer never
    /// modifies the repo or blocks the user's own git commands.
    fn command(&self) -> Command {
        let mut cmd = Command::new("git");
        cmd.arg("--literal-pathspecs")
            .arg("-C")
            .arg(&self.dir)
            .env("LC_ALL", "C")
            .env("GIT_OPTIONAL_LOCKS", "0");
        cmd
    }

    /// Run a read-only git command in `dir`. Exit codes listed in `ok_codes`
    /// besides 0 count as success.
    fn run(&self, args: &[&str], ok_codes: &[i32]) -> Result<Vec<u8>, GitError> {
        let out = self
            .command()
            .args(args)
            .output()
            .map_err(|e| GitError::Failed(format!("could not run git: {e}")))?;
        let code = out.status.code();
        if out.status.success() || code.is_some_and(|c| ok_codes.contains(&c)) {
            return Ok(out.stdout);
        }
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("not a git repository") {
            return Err(GitError::NotARepo);
        }
        let msg = stderr.trim();
        Err(GitError::Failed(if msg.is_empty() {
            format!("git {} failed", args.first().copied().unwrap_or(""))
        } else {
            msg.to_string()
        }))
    }

    fn run_text(&self, args: &[&str], ok_codes: &[i32]) -> Result<String, GitError> {
        Ok(String::from_utf8_lossy(&self.run(args, ok_codes)?).into_owned())
    }
}

impl Git for GitCli {
    fn head(&self) -> Result<Baseline, GitError> {
        self.run(&["rev-parse", "--git-dir"], &[])?;
        // With --quiet an unresolvable HEAD exits 1 silently; any other
        // failure (bad config, damaged repo) is a real error.
        let hash = self.run_text(&["rev-parse", "--verify", "--quiet", "HEAD"], &[1])?;
        let hash = hash.trim();
        if !hash.is_empty() {
            return Ok(Baseline::Commit(hash.to_string()));
        }
        // Unborn only if HEAD names a branch whose ref genuinely does not
        // exist; a corrupt ref also fails to resolve and must be an error.
        let target = self.run_text(&["symbolic-ref", "--quiet", "HEAD"], &[])?;
        let target = target.trim();
        self.run(&["show-ref", "--verify", "--quiet", target], &[1])?;
        if self.run(&["show-ref", "--verify", "--quiet", target], &[]).is_ok() {
            return Err(GitError::Failed(format!("cannot resolve {target}")));
        }
        Ok(Baseline::Empty)
    }

    fn baseline_exists(&self, base: &Baseline) -> bool {
        match base {
            Baseline::Empty => true,
            Baseline::Commit(hash) => {
                let spec = format!("{hash}^{{commit}}");
                self.run(&["cat-file", "-e", &spec], &[]).is_ok()
            }
        }
    }

    fn changes(&self, base: &Baseline) -> Result<Vec<Change>, GitError> {
        // Outside a repo `git diff` silently falls back to --no-index mode.
        self.run(&["rev-parse", "--git-dir"], &[])?;
        let tracked = self.run_text(
            &["diff", "--name-status", "-z", "--no-renames", "--relative", base.rev(), "--", "."],
            &[],
        )?;
        let untracked = self.run_text(&["ls-files", "--others", "--exclude-standard", "-z"], &[])?;
        let mut changes = parse_name_status_z(&tracked);
        for change in parse_untracked_z(&untracked) {
            if !changes.iter().any(|c| c.path == change.path) {
                changes.push(change);
            }
        }
        Ok(changes)
    }

    fn diff(&self, base: &Baseline, change: &Change) -> Result<String, GitError> {
        let bytes = if change.kind == ChangeKind::Untracked {
            // --no-index exits 1 when the files differ.
            self.run(&["diff", "--no-color", "--no-index", "--", "/dev/null", &change.path], &[1])?
        } else {
            self.run(
                &["diff", "--no-color", "--no-renames", "--relative", base.rev(), "--", &change.path],
                &[],
            )?
        };
        let mut text = String::from_utf8_lossy(&bytes).into_owned();
        if text.len() > MAX_DIFF_BYTES {
            let mut cut = MAX_DIFF_BYTES;
            while !text.is_char_boundary(cut) {
                cut -= 1;
            }
            text.truncate(cut);
            if let Some(nl) = text.rfind('\n') {
                text.truncate(nl + 1);
            }
            text.push_str("... diff truncated");
        }
        Ok(text)
    }

    fn log(&self, limit: usize) -> Result<Vec<Commit>, GitError> {
        if self.head()? == Baseline::Empty {
            return Ok(Vec::new());
        }
        let n = limit.to_string();
        let out = self.run_text(
            &["log", "-n", &n, "--date=unix", "--format=%H%x09%h%x09%ct%x09%D%x09%s"],
            &[],
        )?;
        Ok(parse_log(&out))
    }

    fn modified(&self, path: &str) -> Option<SystemTime> {
        std::fs::metadata(self.dir.join(path)).ok()?.modified().ok()
    }
}

#[cfg(test)]
mod tests;
