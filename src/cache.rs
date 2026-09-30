//! Best-effort storage of complete analyses. The scanner owns freshness checks.

use std::{
    env,
    ffi::OsString,
    fs::File,
    io::{self, Read},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

use crate::{repository::Repository, scanner::Inventory, storage::atomic_write};

// Bump when discovery or metadata semantics change without a package version bump.
const FORMAT_VERSION: u32 = 1;
const MAX_CACHE_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct ScanCache {
    root: Option<PathBuf>,
}

#[derive(Serialize, Deserialize)]
struct Entry<T> {
    format_version: u32,
    scanner_version: String,
    inventory: T,
}

impl ScanCache {
    /// Use an explicit directory, shared by all scanner processes using it.
    pub fn new(root: PathBuf) -> Self {
        Self { root: Some(root) }
    }

    pub fn disabled() -> Self {
        Self { root: None }
    }

    /// SKILL_SCANNER_CACHE_DIR overrides the platform cache directory; an empty
    /// value disables persistence. No directory is created until a scan succeeds.
    pub fn from_environment() -> Self {
        Self {
            root: cache_root(|name| env::var_os(name)),
        }
    }

    fn path(&self, repository: &Repository) -> Option<PathBuf> {
        // Prefixes also avoid reserved Windows device names. Validate components
        // even for callers that construct Repository directly instead of parsing.
        let safe = |part: &str| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        };
        if !safe(&repository.owner) || !safe(&repository.name) {
            return None;
        }
        Some(
            self.root
                .as_ref()?
                .join(format!("owner-{}", repository.owner.to_ascii_lowercase()))
                .join(format!(
                    "repo-{}.json",
                    repository.name.to_ascii_lowercase()
                )),
        )
    }

    pub(crate) fn load(&self, repository: &Repository, commit: &str) -> Option<Inventory> {
        let file = File::open(self.path(repository)?).ok()?;
        let metadata = file.metadata().ok()?;
        if !metadata.is_file() || metadata.len() > MAX_CACHE_BYTES {
            return None;
        }
        let mut bytes = Vec::new();
        file.take(MAX_CACHE_BYTES + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() as u64 > MAX_CACHE_BYTES {
            return None;
        }
        let entry: Entry<Inventory> = serde_json::from_slice(&bytes).ok()?;
        let mut inventory = entry.inventory;
        if entry.format_version != FORMAT_VERSION
            || entry.scanner_version != env!("CARGO_PKG_VERSION")
            || inventory.commit.as_deref() != Some(commit)
            || !inventory
                .repository
                .owner
                .eq_ignore_ascii_case(&repository.owner)
            || !inventory
                .repository
                .name
                .eq_ignore_ascii_case(&repository.name)
        {
            return None;
        }
        let mut previous_path = None;
        for skill in &inventory.skills {
            if skill.path.contains('\0')
                || skill
                    .path
                    .split('/')
                    .any(|part| matches!(part, "" | "." | ".."))
                || skill.path.rsplit('/').next() != Some("SKILL.md")
                || previous_path.is_some_and(|previous| previous >= skill.path.as_str())
                || skill.warnings.len() > 2
                || skill
                    .warnings
                    .iter()
                    .any(|warning| !matches!(warning.field.as_str(), "name" | "description"))
                || (skill.warnings.len() == 2 && skill.warnings[0].field == skill.warnings[1].field)
            {
                return None;
            }
            previous_path = Some(skill.path.as_str());
        }
        for skill in &mut inventory.skills {
            // Links and root fallback names follow this request's repository
            // spelling, including when a differently cased input hits the cache.
            skill.link = repository.file_url(commit, &skill.path);
            if skill.path == "SKILL.md"
                && skill.warnings.iter().any(|warning| warning.field == "name")
            {
                skill.name.clone_from(&repository.name);
            }
        }
        inventory.repository = repository.clone();
        Some(inventory)
    }

    pub(crate) fn store(&self, inventory: &Inventory) {
        // Cache failures cannot turn a complete analysis into a failed scan.
        let _ = self.write(inventory);
    }

    fn write(&self, inventory: &Inventory) -> io::Result<()> {
        let Some(path) = self
            .path(&inventory.repository)
            .filter(|_| inventory.commit.is_some())
        else {
            return Ok(());
        };
        let entry = Entry {
            format_version: FORMAT_VERSION,
            scanner_version: env!("CARGO_PKG_VERSION").to_owned(),
            inventory,
        };
        let bytes = serde_json::to_vec(&entry)?;
        if bytes.len() as u64 > MAX_CACHE_BYTES {
            return Ok(());
        }
        atomic_write(&path, &bytes)
    }
}

fn cache_root(get: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    if let Some(path) = get("SKILL_SCANNER_CACHE_DIR") {
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
        nonempty_path("HOME").map(|home| home.join("Library/Caches"))
    } else {
        nonempty_path("XDG_CACHE_HOME")
            .filter(|path| path.is_absolute())
            .or_else(|| nonempty_path("HOME").map(|home| home.join(".cache")))
    };
    base.map(|path| path.join("skill-scanner/scans"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_location_can_be_overridden_disabled_or_unavailable() {
        let custom = PathBuf::from("custom-cache");
        assert_eq!(
            cache_root(|name| (name == "SKILL_SCANNER_CACHE_DIR").then(|| custom.clone().into())),
            Some(custom)
        );
        assert_eq!(cache_root(|_| Some(OsString::new())), None);
        assert_eq!(cache_root(|_| None), None);
    }

    #[test]
    fn chooses_the_platform_cache_directory() {
        let root = env::temp_dir();
        let get = |name: &str| {
            matches!(name, "HOME" | "LOCALAPPDATA" | "XDG_CACHE_HOME").then(|| root.clone().into())
        };
        let base = if cfg!(target_os = "macos") {
            root.join("Library/Caches")
        } else {
            root.clone()
        };
        assert_eq!(cache_root(get), Some(base.join("skill-scanner/scans")));
        if !cfg!(any(target_os = "windows", target_os = "macos")) {
            assert_eq!(
                cache_root(|name| match name {
                    "HOME" => Some(root.clone().into()),
                    "XDG_CACHE_HOME" => Some("relative-path".into()),
                    _ => None,
                }),
                Some(root.join(".cache/skill-scanner/scans"))
            );
        }
    }
}
