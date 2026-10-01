//! Persistent starred skills, independent of scans, history, and the analysis cache.

use std::{
    env,
    ffi::OsString,
    io,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    recent::MAX_JAVASCRIPT_TIME,
    repository::Repository,
    storage::{atomic_write, read_entries, remove_entry, repository_stem, state_root},
};

const FORMAT_VERSION: u32 = 1;
/// Fits the longest path and name even when every character needs JSON escaping.
const MAX_ENTRY_BYTES: u64 = 16 * 1024;
/// Longer repository paths cannot be starred.
pub const MAX_PATH_BYTES: usize = 1024;
/// Longer display names are shortened when starred.
pub const MAX_NAME_CHARS: usize = 200;

#[derive(Clone, Debug)]
pub struct StarredSkills {
    root: Option<PathBuf>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct StarredSkill {
    pub repository: String,
    pub path: String,
    pub name: String,
    /// The scanned commit when the skill was starred; `link` is pinned to it.
    pub commit: String,
    pub link: String,
    /// UTC milliseconds since the Unix epoch.
    pub starred_at: u64,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    format_version: u32,
    repository: String,
    path: String,
    name: String,
    commit: String,
    starred_at: u64,
}

impl StarredSkills {
    pub fn new(root: PathBuf) -> Self {
        Self { root: Some(root) }
    }

    pub fn disabled() -> Self {
        Self { root: None }
    }

    pub fn from_environment() -> Self {
        Self {
            root: starred_root(|name| env::var_os(name)),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.root.is_some()
    }

    /// One file per skill, so concurrent stars never overwrite each other. The
    /// fixed-width path hash keeps names short and safe on every filesystem;
    /// listing verifies the stored repository and path.
    fn filename(repository: &Repository, path: &str) -> io::Result<String> {
        validate_path(path)?;
        Ok(format!(
            "{}_skill-{:016x}.json",
            repository_stem(repository)?,
            fnv1a(path.as_bytes())
        ))
    }

    /// Star a skill from a completed scan, replacing any existing star for the
    /// same repository and path.
    pub fn star(
        &self,
        repository: &Repository,
        path: &str,
        name: &str,
        commit: &str,
    ) -> io::Result<()> {
        let starred_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_millis() as u64;
        self.star_at(repository, path, name, commit, starred_at)
    }

    fn star_at(
        &self,
        repository: &Repository,
        path: &str,
        name: &str,
        commit: &str,
        starred_at: u64,
    ) -> io::Result<()> {
        let filename = Self::filename(repository, path)?;
        if name.is_empty() || !valid_commit(commit) {
            return Err(invalid("Send the skill name and full scanned commit."));
        }
        let Some(root) = &self.root else {
            return Ok(());
        };
        let entry = Entry {
            format_version: FORMAT_VERSION,
            repository: repository.to_string(),
            path: path.to_owned(),
            name: name.chars().take(MAX_NAME_CHARS).collect(),
            commit: commit.to_owned(),
            starred_at,
        };
        atomic_write(&root.join(filename), &serde_json::to_vec(&entry)?)
    }

    /// Read disk on every call so other CLI/server processes are visible.
    pub fn list(&self) -> io::Result<Vec<StarredSkill>> {
        let Some(root) = &self.root else {
            return Ok(Vec::new());
        };
        let mut skills = Vec::new();
        for (filename, bytes) in read_entries(root, MAX_ENTRY_BYTES)? {
            let Ok(entry) = serde_json::from_slice::<Entry>(&bytes) else {
                continue;
            };
            let Ok(repository) = format!("{}.git", entry.repository).parse::<Repository>() else {
                continue;
            };
            if entry.format_version != FORMAT_VERSION
                || repository.to_string() != entry.repository
                || Self::filename(&repository, &entry.path).ok().as_deref()
                    != Some(filename.as_str())
                || entry.name.is_empty()
                || entry.name.chars().count() > MAX_NAME_CHARS
                || !valid_commit(&entry.commit)
                || entry.starred_at > MAX_JAVASCRIPT_TIME
            {
                continue;
            }
            skills.push(StarredSkill {
                link: repository.file_url(&entry.commit, &entry.path),
                repository: entry.repository,
                path: entry.path,
                name: entry.name,
                commit: entry.commit,
                starred_at: entry.starred_at,
            });
        }
        skills.sort_by(|a, b| {
            b.starred_at
                .cmp(&a.starred_at)
                .then_with(|| {
                    a.repository
                        .to_ascii_lowercase()
                        .cmp(&b.repository.to_ascii_lowercase())
                })
                .then_with(|| a.path.cmp(&b.path))
        });
        Ok(skills)
    }

    /// Remove a star; the skill can be starred again from a later scan.
    pub fn unstar(&self, repository: &Repository, path: &str) -> io::Result<()> {
        let filename = Self::filename(repository, path)?;
        let Some(root) = &self.root else {
            return Ok(());
        };
        remove_entry(root, &filename, "Starred skills")
    }
}

/// Accept only repository-relative SKILL.md paths that a scan can report.
fn validate_path(path: &str) -> io::Result<()> {
    if path.is_empty()
        || path.len() > MAX_PATH_BYTES
        || !(path == "SKILL.md" || path.ends_with("/SKILL.md"))
        || path
            .split('/')
            .any(|segment| matches!(segment, "" | "." | ".."))
    {
        return Err(invalid(&format!(
            "Expected a repository-relative SKILL.md path of at most {MAX_PATH_BYTES} bytes."
        )));
    }
    Ok(())
}

fn valid_commit(commit: &str) -> bool {
    matches!(commit.len(), 40 | 64)
        && commit
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

/// 64-bit FNV-1a: stable across platforms and releases, unlike std's hasher.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn starred_root(get: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    state_root(get, "SKILL_SCANNER_STARRED_DIR", "starred")
}

#[cfg(test)]
#[path = "starred_tests.rs"]
mod tests;
