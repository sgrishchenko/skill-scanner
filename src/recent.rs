//! Persistent repository summaries, independent of the disposable analysis cache.

use std::{
    env,
    ffi::OsString,
    fs::{self, File},
    io::{self, Read},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{repository::Repository, scanner::Inventory, storage::atomic_write};

const FORMAT_VERSION: u32 = 1;
const MAX_ENTRY_BYTES: u64 = 4096;

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
        // Validate even when Repository was constructed directly. A flat,
        // prefixed filename is safe on all supported filesystems. Append a URL
        // suffix so parsing preserves repository names that themselves end in .git.
        let parsed = format!("{repository}.git").parse::<Repository>();
        if parsed.as_ref() != Ok(repository) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid repository.",
            ));
        }
        Ok(format!(
            "owner-{}_repo-{}.json",
            repository.owner.to_ascii_lowercase(),
            repository.name.to_ascii_lowercase()
        ))
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
        let files = match fs::read_dir(root) {
            Ok(files) => files,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let mut entries = Vec::new();
        for file in files {
            let file = file?;
            let path = file.path();
            if path.extension().is_none_or(|extension| extension != "json")
                || !file.file_type()?.is_file()
            {
                continue;
            }
            let file = match File::open(&path) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            if file.metadata()?.len() > MAX_ENTRY_BYTES {
                continue;
            }
            let mut bytes = Vec::new();
            file.take(MAX_ENTRY_BYTES + 1).read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_ENTRY_BYTES {
                continue;
            }
            let Ok(entry) = serde_json::from_slice::<Entry>(&bytes) else {
                continue;
            };
            let Ok(repository) = format!("{}.git", entry.recent.repository).parse::<Repository>()
            else {
                continue;
            };
            if entry.format_version != FORMAT_VERSION
                || repository.to_string() != entry.recent.repository
                || Self::filename(&repository).ok().as_deref()
                    != path.file_name().and_then(|name| name.to_str())
                // The browser must be able to format this as a JavaScript Date.
                || entry.recent.scanned_at > 8_640_000_000_000_000
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
        match fs::remove_file(root.join(Self::filename(repository)?)) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }
}

fn history_root(get: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    if let Some(path) = get("SKILL_SCANNER_HISTORY_DIR") {
        return (!path.is_empty()).then(|| PathBuf::from(path));
    }
    let nonempty_path = |name| {
        get(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    let base = if cfg!(target_os = "windows") {
        nonempty_path("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        nonempty_path("HOME").map(|home| home.join("Library/Application Support"))
    } else {
        nonempty_path("XDG_STATE_HOME")
            .filter(|path| path.is_absolute())
            .or_else(|| nonempty_path("HOME").map(|home| home.join(".local/state")))
    };
    base.map(|path| path.join("skill-scanner/recent"))
}

#[cfg(test)]
#[path = "recent_tests.rs"]
mod tests;
