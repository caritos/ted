//! In-memory `Git` for tests of the git view and the app.

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::SystemTime;

use super::{Baseline, Change, ChangeKind, Commit, Git, GitError};

pub(crate) struct FakeGit {
    pub head: Baseline,
    pub changes: Vec<Change>,
    /// Diff text by path; a missing path diffs as empty.
    pub diffs: HashMap<String, String>,
    pub log: Vec<Commit>,
    /// Working-tree mtimes by path; a missing path has no file.
    pub mtimes: HashMap<String, SystemTime>,
    /// When set, every fallible call returns this error.
    pub fail: Option<GitError>,
    /// What `baseline_exists` reports for a commit baseline.
    pub existing: bool,
    /// Paths passed to `diff`, in call order.
    pub diff_calls: RefCell<Vec<String>>,
}

impl FakeGit {
    pub fn new() -> Self {
        FakeGit {
            head: Baseline::Commit("h1".to_string()),
            changes: Vec::new(),
            diffs: HashMap::new(),
            log: Vec::new(),
            mtimes: HashMap::new(),
            fail: None,
            existing: true,
            diff_calls: RefCell::new(Vec::new()),
        }
    }

    /// Adds a change; `mtime` of `None` means the file is gone from disk.
    pub fn add(&mut self, kind: ChangeKind, path: &str, mtime: Option<SystemTime>) {
        self.changes.push(Change { kind, path: path.to_string() });
        match mtime {
            Some(t) => self.mtimes.insert(path.to_string(), t),
            None => self.mtimes.remove(path),
        };
    }

    pub fn remove(&mut self, path: &str) {
        self.changes.retain(|c| c.path != path);
        self.mtimes.remove(path);
    }

    fn check(&self) -> Result<(), GitError> {
        match &self.fail {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }
}

impl Git for FakeGit {
    fn head(&self) -> Result<Baseline, GitError> {
        self.check()?;
        Ok(self.head.clone())
    }

    fn baseline_exists(&self, base: &Baseline) -> bool {
        match base {
            Baseline::Empty => true,
            Baseline::Commit(_) => self.existing,
        }
    }

    fn changes(&self, _base: &Baseline) -> Result<Vec<Change>, GitError> {
        self.check()?;
        Ok(self.changes.clone())
    }

    fn diff(&self, _base: &Baseline, change: &Change) -> Result<String, GitError> {
        self.check()?;
        self.diff_calls.borrow_mut().push(change.path.clone());
        Ok(self.diffs.get(&change.path).cloned().unwrap_or_default())
    }

    fn log(&self, limit: usize) -> Result<Vec<Commit>, GitError> {
        self.check()?;
        Ok(self.log.iter().take(limit).cloned().collect())
    }

    fn modified(&self, path: &str) -> Option<SystemTime> {
        self.mtimes.get(path).copied()
    }
}
