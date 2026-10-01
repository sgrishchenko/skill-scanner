//! Persistent repository summaries, independent of the disposable analysis cache.

use std::{
    env,
    ffi::OsString,
    io,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    repository::Repository,
    scanner::Inventory,
    storage::{atomic_write, read_entries, remove_entry, repository_stem, state_root},
};

const FORMAT_VERSION: u32 = 1;
const MAX_ENTRY_BYTES: u64 = 4096;
/// The latest time the browser can format as a JavaScript Date.
pub(crate) const MAX_JAVASCRIPT_TIME: u64 = 8_640_000_000_000_000;

#[derive(Clone, Debug)]
pub struct RecentRepositories {
    root: Option<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecentRepository {
    pub repository: String,
    /// UTC milliseconds since the Unix epoch.
    pub scanned_at: u64,
    pub skill_count: usize,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    format_version: u32,
    #[serde(flatten)]
    recent: RecentRepository,
}

impl RecentRepositories {
    pub fn new(root: PathBuf) -> Self {
        Self { root: Some(root) }
    }

    pub fn disabled() -> Self {
        Self { root: None }
    }

    pub fn from_environment() -> Self {
        Self {
            root: history_root(|name| env::var_os(name)),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.root.is_some()
    }

    fn filename(repository: &Repository) -> io::Result<String> {
        Ok(format!("{}.json", repository_stem(repository)?))
    }

    pub fn record(&self, inventory: &Inventory) -> io::Result<()> {
        let scanned_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_millis() as u64;
        self.record_at(inventory, scanned_at)
    }

    fn record_at(&self, inventory: &Inventory, scanned_at: u64) -> io::Result<()> {
        let Some(root) = &self.root else {
            return Ok(());
        };
        let path = root.join(Self::filename(&inventory.repository)?);
        let entry = Entry {
            format_version: FORMAT_VERSION,
            recent: RecentRepository {
                repository: inventory.repository.to_string(),
                scanned_at,
                skill_count: inventory.skills.len(),
            },
        };
        atomic_write(&path, &serde_json::to_vec(&entry)?)
    }

    /// Read disk on every call so other CLI/server processes are visible.
    pub fn list(&self) -> io::Result<Vec<RecentRepository>> {
        let Some(root) = &self.root else {
            return Ok(Vec::new());
        };
        let mut entries = Vec::new();
        for (filename, bytes) in read_entries(root, MAX_ENTRY_BYTES)? {
            let Ok(entry) = serde_json::from_slice::<Entry>(&bytes) else {
                continue;
            };
            let Ok(repository) = format!("{}.git", entry.recent.repository).parse::<Repository>()
            else {
                continue;
            };
            if entry.format_version != FORMAT_VERSION
                || repository.to_string() != entry.recent.repository
                || Self::filename(&repository).ok().as_deref() != Some(filename.as_str())
                || entry.recent.scanned_at > MAX_JAVASCRIPT_TIME
            {
                continue;
            }
            entries.push(entry.recent);
        }
        entries.sort_by(|a, b| {
            b.scanned_at.cmp(&a.scanned_at).then_with(|| {
                a.repository
                    .to_ascii_lowercase()
                    .cmp(&b.repository.to_ascii_lowercase())
            })
        });
        Ok(entries)
    }

    /// Forget a summary; a later successful scan can add it again.
    pub fn remove(&self, repository: &Repository) -> io::Result<()> {
        let Some(root) = &self.root else {
            return Ok(());
        };
        remove_entry(
            root,
            &Self::filename(repository)?,
            "Recent repository history",
        )
    }
}

fn history_root(get: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    state_root(get, "SKILL_SCANNER_HISTORY_DIR", "recent")
}

#[cfg(test)]
#[path = "recent_tests.rs"]
mod tests;
