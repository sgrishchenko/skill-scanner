//! Small versioned JSON entries shared by recent repositories and starred skills.

use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::repository::Repository;

static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);

/// Publish a complete file without exposing partial JSON to other processes.
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    fs::create_dir_all(path.parent().expect("stored entry has a parent"))?;
    let temporary = path.with_extension(format!(
        "{}-{}.tmp",
        std::process::id(),
        NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let written = file.write_all(bytes).and_then(|_| file.sync_all());
    drop(file);
    let result = written.and_then(|_| fs::rename(&temporary, path));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// Resolve a per-user state directory. A nonempty override replaces the whole
/// directory and an empty override disables storage.
pub(crate) fn state_root(
    get: impl Fn(&str) -> Option<OsString>,
    override_name: &str,
    leaf: &str,
) -> Option<PathBuf> {
    if let Some(path) = get(override_name) {
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
    base.map(|path| path.join("skill-scanner").join(leaf))
}

/// A flat, case-insensitive filename prefix for one repository.
pub(crate) fn repository_stem(repository: &Repository) -> io::Result<String> {
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
        "owner-{}_repo-{}",
        repository.owner.to_ascii_lowercase(),
        repository.name.to_ascii_lowercase()
    ))
}

/// Read every JSON entry of at most `max_bytes`, reading disk on every call so
/// other CLI/server processes are visible. Oversized and vanished entries are
/// skipped; a missing directory has no entries.
pub(crate) fn read_entries(root: &Path, max_bytes: u64) -> io::Result<Vec<(String, Vec<u8>)>> {
    let files = match fs::read_dir(root) {
        Ok(files) => files,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut entries = Vec::new();
    for file in files {
        let file = file?;
        let path = file.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if path.extension().is_none_or(|extension| extension != "json")
            || !file.file_type()?.is_file()
        {
            continue;
        }
        if let Some(bytes) = read_bounded(&path, max_bytes)? {
            entries.push((name.to_owned(), bytes));
        }
    }
    Ok(entries)
}

/// Read one file of at most `max_bytes`. Oversized and vanished files are `None`.
pub(crate) fn read_bounded(path: &Path, max_bytes: u64) -> io::Result<Option<Vec<u8>>> {
    let file = match open_entry(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if file.metadata()?.len() > max_bytes {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    file.take(max_bytes + 1).read_to_end(&mut bytes)?;
    Ok((bytes.len() as u64 <= max_bytes).then_some(bytes))
}

/// Remove one entry. Only a missing entry or directory is a successful no-op.
pub(crate) fn remove_entry(root: &Path, filename: &str, description: &str) -> io::Result<()> {
    match fs::remove_file(root.join(filename)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            // Windows also reports NotFound when root is a regular file.
            match fs::metadata(root) {
                Ok(metadata) if !metadata.is_dir() => Err(io::Error::new(
                    io::ErrorKind::NotADirectory,
                    format!("{description} path is not a directory."),
                )),
                Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
                _ => Ok(()),
            }
        }
        result => result,
    }
}

fn open_entry(path: &Path) -> io::Result<File> {
    #[cfg(windows)]
    {
        retry_windows_entry_open(|| File::open(path), std::thread::sleep)
    }
    #[cfg(not(windows))]
    {
        File::open(path)
    }
}

#[cfg(any(windows, test))]
fn retry_windows_entry_open<T>(
    mut open: impl FnMut() -> io::Result<T>,
    mut wait: impl FnMut(std::time::Duration),
) -> io::Result<T> {
    // Replacing an entry can briefly leave the destination pending deletion
    // on Windows. CreateFile may report ACCESS_DENIED, SHARING_VIOLATION, or
    // DELETE_PENDING during that interval. Persistent errors still reach callers.
    let mut delays = [5, 10, 20].into_iter();
    loop {
        match open() {
            Err(error) if matches!(error.raw_os_error(), Some(5 | 32 | 303)) => {
                let Some(delay) = delays.next() else {
                    return Err(error);
                };
                wait(std::time::Duration::from_millis(delay));
            }
            result => return result,
        }
    }
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
