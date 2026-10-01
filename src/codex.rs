//! Install scanned skills into Codex's personal skills directory and remove them.

use std::{
    collections::HashSet,
    env,
    ffi::OsString,
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    github::{BlobContent, GitHubClient, TreeEntry},
    recent::MAX_JAVASCRIPT_TIME,
    repository::Repository,
    scanner::{is_regular_file, validate_entries},
    starred::{valid_commit, validate_path},
    storage::read_bounded,
    ScanError,
};

const FORMAT_VERSION: u32 = 1;
/// Records the source of a folder installed by Skill Scanner. Folders without
/// a valid record are never listed, replaced, or removed.
pub const MARKER: &str = ".skill-scanner.json";
const MAX_MARKER_BYTES: u64 = 16 * 1024;
/// Larger skills cannot be installed.
pub const MAX_FILES: usize = 1000;
pub const MAX_INSTALL_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_NAME_BYTES: usize = 100;

static NEXT_TEMP_DIRECTORY: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub struct CodexSkills {
    root: Option<PathBuf>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct InstalledSkill {
    /// The skill's folder name in the Codex skills directory.
    pub name: String,
    pub repository: String,
    pub path: String,
    /// The commit the files were downloaded from; `link` is pinned to it.
    pub commit: String,
    pub link: String,
    /// UTC milliseconds since the Unix epoch.
    pub installed_at: u64,
}

#[derive(Debug)]
pub struct Installation {
    pub skill: InstalledSkill,
    pub directory: PathBuf,
    pub files: usize,
}

#[derive(Serialize, Deserialize)]
struct Marker {
    format_version: u32,
    repository: String,
    path: String,
    commit: String,
    installed_at: u64,
}

#[derive(Debug)]
pub enum InstallProgress<'a> {
    Repository(&'a Repository),
    Commit(&'a str),
    Listing(&'a str),
    File {
        path: &'a str,
        current: usize,
        total: usize,
    },
}

#[derive(Debug)]
pub enum CodexError {
    /// An invalid repository path, folder name, or commit.
    Invalid(String),
    Disabled,
    /// The folder exists but was not installed from the requested skill.
    Conflict(String),
    /// GitHub access failed, or the skill's files cannot be installed safely.
    Source(ScanError),
    Storage(io::Error),
}

impl fmt::Display for CodexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) | Self::Conflict(message) => f.write_str(message),
            Self::Disabled => f.write_str("Codex skill installation is disabled. Set SKILL_SCANNER_CODEX_SKILLS_DIR to a directory to enable it."),
            Self::Source(error) => write!(f, "{error}"),
            Self::Storage(_) => f.write_str("Could not access the Codex skills directory. Check SKILL_SCANNER_CODEX_SKILLS_DIR and directory permissions."),
        }
    }
}

impl std::error::Error for CodexError {}

impl From<ScanError> for CodexError {
    fn from(error: ScanError) -> Self {
        Self::Source(error)
    }
}

impl From<io::Error> for CodexError {
    fn from(error: io::Error) -> Self {
        Self::Storage(error)
    }
}

fn unsupported(message: &str) -> CodexError {
    CodexError::Source(ScanError::new(message))
}

impl CodexSkills {
    pub fn new(root: PathBuf) -> Self {
        Self { root: Some(root) }
    }

    pub fn disabled() -> Self {
        Self { root: None }
    }

    pub fn from_environment() -> Self {
        Self {
            root: codex_root(|name| env::var_os(name))
                .map(|root| std::path::absolute(&root).unwrap_or(root)),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.root.is_some()
    }

    pub fn directory(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// Download a skill's directory from GitHub and publish it as one folder.
    /// Without a commit, the default branch's current commit is installed.
    /// Reinstalling the same skill replaces its folder; other folders are kept.
    pub fn install(
        &self,
        client: &GitHubClient,
        repository: &Repository,
        path: &str,
        commit: Option<&str>,
        mut progress: impl FnMut(InstallProgress<'_>),
    ) -> Result<Installation, CodexError> {
        let name = install_name(repository, path)?;
        if commit.is_some_and(|commit| !valid_commit(commit)) {
            return Err(CodexError::Invalid(
                "Send the full scanned commit of the skill.".to_owned(),
            ));
        }
        let root = self.root.as_ref().ok_or(CodexError::Disabled)?;
        // Fail before any GitHub request when the folder holds something else.
        replaceable(root, &name, repository, path)?;
        progress(InstallProgress::Repository(repository));
        let info = client.repository(repository)?;
        if info.private {
            return Err(unsupported(
                "Private repositories are unsupported; choose one public GitHub repository.",
            ));
        }
        let reference = commit.unwrap_or(&info.default_branch);
        if reference.is_empty() {
            return Err(unsupported(
                "GitHub did not identify a default branch; retry the installation.",
            ));
        }
        progress(InstallProgress::Commit(reference));
        let Some(resolved) = client.default_commit(repository, reference)? else {
            return Err(unsupported(
                "The repository is empty; there is no skill to install.",
            ));
        };
        if commit.is_some_and(|commit| commit != resolved.sha) {
            return Err(unsupported(
                "GitHub returned a different commit; retry the installation.",
            ));
        }
        let directory = path
            .strip_suffix("SKILL.md")
            .expect("validated SKILL.md path")
            .trim_end_matches('/');
        progress(InstallProgress::Listing(if directory.is_empty() {
            "/"
        } else {
            directory
        }));
        let files = list_files(client, repository, &resolved.commit.tree.sha, directory)?;
        let mut downloaded = Vec::with_capacity(files.len());
        let mut remaining = MAX_INSTALL_BYTES;
        for (index, entry) in files.iter().enumerate() {
            let source = if directory.is_empty() {
                entry.path.clone()
            } else {
                format!("{directory}/{}", entry.path)
            };
            progress(InstallProgress::File {
                path: &source,
                current: index + 1,
                total: files.len(),
            });
            let bytes = match client
                .blob_up_to(repository, &entry.sha, remaining)
                .map_err(|error| error.context(&source))?
            {
                BlobContent::Bytes(bytes) => bytes,
                BlobContent::TooLarge => return Err(too_large()),
            };
            if entry.size.is_some_and(|size| size != bytes.len() as u64) {
                return Err(unsupported(&format!(
                    "{source}: GitHub returned inconsistent file sizes; retry the installation."
                )));
            }
            remaining -= bytes.len() as u64;
            downloaded.push((entry, bytes));
        }
        let installed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_millis() as u64;
        let marker = Marker {
            format_version: FORMAT_VERSION,
            repository: repository.to_string(),
            path: path.to_owned(),
            commit: resolved.sha,
            installed_at,
        };
        publish(
            root,
            &name,
            repository,
            path,
            &downloaded,
            &serde_json::to_vec(&marker).map_err(io::Error::from)?,
        )?;
        Ok(Installation {
            directory: root.join(&name),
            files: downloaded.len(),
            skill: InstalledSkill {
                link: repository.file_url(&marker.commit, path),
                name,
                repository: marker.repository,
                path: marker.path,
                commit: marker.commit,
                installed_at,
            },
        })
    }

    /// Read disk on every call so other CLI/server processes are visible.
    /// Only folders installed by Skill Scanner are listed.
    pub fn list(&self) -> io::Result<Vec<InstalledSkill>> {
        let Some(root) = &self.root else {
            return Ok(Vec::new());
        };
        let entries = match fs::read_dir(root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let mut skills = Vec::new();
        for entry in entries {
            let entry = entry?;
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            if !name.starts_with('.') && entry.file_type()?.is_dir() {
                skills.extend(read_marker(&entry.path(), &name));
            }
        }
        skills.sort_by(|a, b| {
            a.name
                .to_ascii_lowercase()
                .cmp(&b.name.to_ascii_lowercase())
                .then_with(|| a.name.cmp(&b.name))
        });
        Ok(skills)
    }

    /// Delete a folder installed by Skill Scanner. A missing folder is `None`.
    pub fn remove(&self, name: &str) -> Result<Option<InstalledSkill>, CodexError> {
        validate_name(name)?;
        let root = self.root.as_ref().ok_or(CodexError::Disabled)?;
        let target = root.join(name);
        if !exists(root, &target)? {
            return Ok(None);
        }
        let Some(installed) = read_marker(&target, name) else {
            return Err(CodexError::Conflict(format!(
                "The Codex skill folder {name} was not installed by Skill Scanner, so it was left unchanged."
            )));
        };
        let removed = temporary(root, name, "removed");
        fs::rename(&target, &removed)?;
        if let Err(error) = fs::remove_dir_all(&removed) {
            // Keep the skill visible rather than leave a hidden partial copy.
            let _ = fs::rename(&removed, &target);
            return Err(error.into());
        }
        Ok(Some(installed))
    }
}

/// The folder Codex discovers: the skill's directory name, or the repository
/// name for a root-level SKILL.md.
pub fn install_name(repository: &Repository, path: &str) -> Result<String, CodexError> {
    validate_path(path).map_err(|error| CodexError::Invalid(error.to_string()))?;
    let name = path
        .rsplit_once('/')
        .map(|(directory, _)| directory.rsplit('/').next().expect("directory component"))
        .unwrap_or(&repository.name);
    validate_name(name)?;
    Ok(name.to_owned())
}

/// Folder names are portable, visible, and unambiguous on every supported platform.
fn validate_name(name: &str) -> Result<(), CodexError> {
    let valid = !name.is_empty()
        && name.len() <= MAX_NAME_BYTES
        && name.starts_with(|c: char| c.is_ascii_alphanumeric())
        && !name.ends_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        && !reserved_device(name);
    if !valid {
        return Err(CodexError::Invalid(format!("A Codex skill folder name must start with a letter or digit, use only ASCII letters, digits, '.', '_', or '-', and have at most {MAX_NAME_BYTES} characters.")));
    }
    Ok(())
}

/// Windows reserves these names, with or without an extension.
fn reserved_device(segment: &str) -> bool {
    let stem = segment
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit())
}

/// Repository file names that every supported filesystem stores as written.
fn portable_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment.len() <= 255
        && !matches!(segment, "." | "..")
        && !segment
            .chars()
            .any(|c| c.is_control() || matches!(c, '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        && !segment.ends_with(['.', ' '])
        && !reserved_device(segment)
}

/// List the regular files of the skill's directory at a pinned root tree.
fn list_files(
    client: &GitHubClient,
    repository: &Repository,
    root_sha: &str,
    directory: &str,
) -> Result<Vec<TreeEntry>, CodexError> {
    let mut sha = root_sha.to_owned();
    for segment in directory.split('/').filter(|segment| !segment.is_empty()) {
        let tree = client.tree(repository, &sha, false)?;
        if tree.truncated {
            return Err(unsupported(
                "GitHub truncated a directory listing; the skill cannot be installed.",
            ));
        }
        validate_entries(&tree.tree, false)?;
        sha = tree
            .tree
            .into_iter()
            .find(|entry| entry.kind == "tree" && entry.path == segment)
            .ok_or_else(|| {
                unsupported("The skill's directory does not exist at this commit; scan the repository again.")
            })?
            .sha;
    }
    let tree = client.tree(repository, &sha, true)?;
    if tree.truncated {
        return Err(too_large());
    }
    validate_entries(&tree.tree, true)?;
    select_files(tree.tree)
}

/// Keep regular files, excluding symlinks, submodules, nested skills (separate
/// directories with their own SKILL.md), and a copied installation record.
fn select_files(entries: Vec<TreeEntry>) -> Result<Vec<TreeEntry>, CodexError> {
    if !entries
        .iter()
        .any(|entry| entry.path == "SKILL.md" && is_regular_file(entry))
    {
        return Err(unsupported(
            "The skill's SKILL.md is not a regular file at this commit; scan the repository again.",
        ));
    }
    let nested: Vec<&str> = entries
        .iter()
        .filter(|entry| is_regular_file(entry))
        .filter_map(|entry| entry.path.strip_suffix("SKILL.md"))
        .filter(|prefix| prefix.ends_with('/'))
        .collect();
    let files: Vec<TreeEntry> = entries
        .iter()
        .filter(|entry| {
            is_regular_file(entry)
                && entry.path != MARKER
                && !nested.iter().any(|prefix| entry.path.starts_with(prefix))
        })
        .cloned()
        .collect();
    if files.len() > MAX_FILES
        || files
            .iter()
            .map(|entry| entry.size.unwrap_or(0))
            .sum::<u64>()
            > MAX_INSTALL_BYTES
    {
        return Err(too_large());
    }
    let mut folded = HashSet::new();
    for entry in &files {
        // Case-insensitive filesystems would merge paths differing only in case.
        if !entry.path.split('/').all(portable_segment) || !folded.insert(entry.path.to_lowercase())
        {
            return Err(unsupported(&format!(
                "The skill contains a file name that cannot be installed safely on every platform: {}",
                entry.path
            )));
        }
    }
    Ok(files)
}

fn too_large() -> CodexError {
    unsupported(&format!(
        "The skill is too large to install; skills are limited to {MAX_FILES} files and {} MiB.",
        MAX_INSTALL_BYTES / 1024 / 1024
    ))
}

fn codex_root(get: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    if let Some(path) = get("SKILL_SCANNER_CODEX_SKILLS_DIR") {
        return (!path.is_empty()).then(|| PathBuf::from(path));
    }
    // Codex discovers personal skills in $HOME/.agents/skills.
    let home = if cfg!(target_os = "windows") {
        "USERPROFILE"
    } else {
        "HOME"
    };
    get(home)
        .filter(|path| !path.is_empty())
        .map(|home| PathBuf::from(home).join(".agents").join("skills"))
}

/// Whether the target folder exists. Windows reports a missing entry when the
/// root is a regular file, so check the root separately.
fn exists(root: &Path, target: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(target) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => match fs::metadata(root) {
            Ok(metadata) if !metadata.is_dir() => Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "Codex skills path is not a directory.",
            )),
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(false),
        },
        Err(error) => Err(error),
    }
}

/// Whether the folder is absent (`false`) or holds an earlier installation of
/// the same skill (`true`). Any other folder or file is a conflict.
fn replaceable(
    root: &Path,
    name: &str,
    repository: &Repository,
    path: &str,
) -> Result<bool, CodexError> {
    let target = root.join(name);
    if !exists(root, &target)? {
        return Ok(false);
    }
    match read_marker(&target, name) {
        Some(installed)
            if installed
                .repository
                .eq_ignore_ascii_case(&repository.to_string())
                && installed.path == path =>
        {
            Ok(true)
        }
        Some(installed) => Err(CodexError::Conflict(format!(
            "The Codex skill folder {name} already holds {} from {}. Remove it first to install this skill.",
            installed.path, installed.repository
        ))),
        None => Err(CodexError::Conflict(format!(
            "The Codex skills directory already contains {name}, which was not installed by Skill Scanner. Remove or rename it to install this skill."
        ))),
    }
}

/// Read a folder's installation record, verifying that it names this folder.
fn read_marker(directory: &Path, name: &str) -> Option<InstalledSkill> {
    if !fs::symlink_metadata(directory).ok()?.is_dir() {
        return None;
    }
    let marker = directory.join(MARKER);
    if !fs::symlink_metadata(&marker).ok()?.is_file() {
        return None;
    }
    let bytes = read_bounded(&marker, MAX_MARKER_BYTES).ok()??;
    let marker = serde_json::from_slice::<Marker>(&bytes).ok()?;
    let repository = format!("{}.git", marker.repository)
        .parse::<Repository>()
        .ok()?;
    if marker.format_version != FORMAT_VERSION
        || repository.to_string() != marker.repository
        || install_name(&repository, &marker.path).ok()?.as_str() != name
        || !valid_commit(&marker.commit)
        || marker.installed_at > MAX_JAVASCRIPT_TIME
    {
        return None;
    }
    Some(InstalledSkill {
        name: name.to_owned(),
        link: repository.file_url(&marker.commit, &marker.path),
        repository: marker.repository,
        path: marker.path,
        commit: marker.commit,
        installed_at: marker.installed_at,
    })
}

/// A hidden sibling, so Codex never discovers a partial or replaced copy.
fn temporary(root: &Path, name: &str, purpose: &str) -> PathBuf {
    root.join(format!(
        ".skill-scanner-{name}-{}-{}.{purpose}",
        std::process::id(),
        NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed)
    ))
}

/// Write every file into a hidden staging folder, then rename it into place.
/// A replaced installation is restored if the final rename fails.
fn publish(
    root: &Path,
    name: &str,
    repository: &Repository,
    path: &str,
    files: &[(&TreeEntry, Vec<u8>)],
    marker: &[u8],
) -> Result<(), CodexError> {
    fs::create_dir_all(root)?;
    let staging = temporary(root, name, "installing");
    fs::create_dir(&staging)?;
    let result = write_files(&staging, files, marker)
        .map_err(CodexError::from)
        .and_then(|()| {
            let target = root.join(name);
            // Recheck in case another process changed the folder meanwhile.
            if !replaceable(root, name, repository, path)? {
                return Ok(fs::rename(&staging, &target)?);
            }
            let replaced = temporary(root, name, "replaced");
            fs::rename(&target, &replaced)?;
            if let Err(error) = fs::rename(&staging, &target) {
                let _ = fs::rename(&replaced, &target);
                return Err(error.into());
            }
            let _ = fs::remove_dir_all(&replaced);
            Ok(())
        });
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

fn write_files(staging: &Path, files: &[(&TreeEntry, Vec<u8>)], marker: &[u8]) -> io::Result<()> {
    for (entry, bytes) in files {
        let destination = entry
            .path
            .split('/')
            .fold(staging.to_path_buf(), |path, segment| path.join(segment));
        fs::create_dir_all(destination.parent().expect("file has a parent"))?;
        write_new(&destination, bytes, entry.mode == "100755")?;
    }
    write_new(&staging.join(MARKER), marker, false)
}

/// Create a new file; repository executables stay executable on Unix.
fn write_new(path: &Path, bytes: &[u8], executable: bool) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(if executable { 0o777 } else { 0o666 });
    }
    #[cfg(not(unix))]
    let _ = executable;
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(test)]
#[path = "codex_tests.rs"]
mod tests;
