use std::collections::{HashMap, HashSet};

use crate::{
    cache::ScanCache,
    github::{
        validate_sha, BlobContent, GitHubClient, RepositoryInfo, Tree, TreeEntry, MAX_SKILL_BYTES,
    },
    metadata::{self, Metadata, MetadataWarning},
    recent::RecentRepositories,
    repository::Repository,
    ScanError,
};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Inventory {
    pub repository: Repository,
    pub commit: Option<String>,
    pub skills: Vec<Skill>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Skill {
    pub path: String,
    pub name: String,
    pub description: String,
    pub link: String,
    pub warnings: Vec<MetadataWarning>,
}

#[derive(Debug)]
pub enum ScanProgress<'a> {
    Repository(&'a Repository),
    DefaultBranch(&'a str),
    Cached(&'a str),
    HistoryWarning(&'a str),
    DiscoveringSkills,
    Directory(&'a str),
    Skill {
        path: &'a str,
        current: usize,
        total: usize,
    },
}

pub fn scan(client: &GitHubClient, repository: &Repository) -> Result<Inventory, ScanError> {
    scan_with_progress(client, repository, |_| {})
}

/// Report the current operation before starting potentially slow network work.
pub fn scan_with_progress(
    client: &GitHubClient,
    repository: &Repository,
    progress: impl FnMut(ScanProgress<'_>),
) -> Result<Inventory, ScanError> {
    scan_with_cache(client, repository, &ScanCache::disabled(), progress)
}

/// Both interfaces remember only successful scans, including cache hits and
/// empty repositories. History failures leave the completed inventory usable.
pub fn scan_with_storage(
    client: &GitHubClient,
    repository: &Repository,
    cache: &ScanCache,
    recent: &RecentRepositories,
    mut progress: impl FnMut(ScanProgress<'_>),
) -> Result<Inventory, ScanError> {
    let inventory = scan_with_cache(client, repository, cache, &mut progress)?;
    if recent.record(&inventory).is_err() {
        progress(ScanProgress::HistoryWarning(
            "Scan completed, but the repository could not be saved to recent repositories. Check SKILL_SCANNER_HISTORY_DIR and directory permissions.",
        ));
    }
    Ok(inventory)
}

/// Revalidate the public repository and current commit before reusing analysis.
pub fn scan_with_cache(
    client: &GitHubClient,
    repository: &Repository,
    cache: &ScanCache,
    mut progress: impl FnMut(ScanProgress<'_>),
) -> Result<Inventory, ScanError> {
    progress(ScanProgress::Repository(repository));
    let info = client.repository(repository)?;
    scan_snapshot(client, repository, &info, cache, true, progress)
}

/// Scan a repository whose details were already fetched, either directly or
/// from an organization listing. Without `walk_truncated`, a truncated
/// recursive tree fails instead of requesting every directory separately.
pub(crate) fn scan_snapshot(
    client: &GitHubClient,
    repository: &Repository,
    info: &RepositoryInfo,
    cache: &ScanCache,
    walk_truncated: bool,
    mut progress: impl FnMut(ScanProgress<'_>),
) -> Result<Inventory, ScanError> {
    if info.private {
        return Err(ScanError::new(
            "Private repositories are unsupported; choose one public GitHub repository.",
        ));
    }
    if info.default_branch.is_empty() {
        return Err(ScanError::new(
            "GitHub did not identify a default branch; retry the scan.",
        ));
    }
    let mut inventory = Inventory {
        repository: repository.clone(),
        commit: None,
        skills: Vec::new(),
    };
    progress(ScanProgress::DefaultBranch(&info.default_branch));
    let Some(commit) = client.default_commit(repository, &info.default_branch)? else {
        return Ok(inventory);
    };
    if let Some(inventory) = cache.load(repository, &commit.sha) {
        progress(ScanProgress::Cached(&commit.sha));
        return Ok(inventory);
    }
    let candidates = discover(
        client,
        repository,
        &commit.commit.tree.sha,
        walk_truncated,
        &mut progress,
    )?;
    let total = candidates.len();
    for (index, entry) in candidates.into_iter().enumerate() {
        progress(ScanProgress::Skill {
            path: &entry.path,
            current: index + 1,
            total,
        });
        let fallback = entry
            .path
            .rsplit_once('/')
            .map(|(directory, _)| directory.rsplit('/').next().expect("directory component"))
            .unwrap_or(&repository.name);
        let metadata = if entry.size.is_some_and(|size| size > MAX_SKILL_BYTES) {
            oversized(fallback)
        } else {
            match client
                .blob(repository, &entry.sha)
                .map_err(|error| error.context(&entry.path))?
            {
                BlobContent::Bytes(bytes) => {
                    if entry.size.is_some_and(|size| size != bytes.len() as u64) {
                        return Err(ScanError::new(format!("{}: GitHub returned inconsistent file sizes; the inventory is incomplete. Retry the scan.", entry.path)));
                    }
                    metadata::parse(&bytes, fallback)
                }
                BlobContent::TooLarge => oversized(fallback),
            }
        };
        inventory.skills.push(Skill {
            link: repository.file_url(&commit.sha, &entry.path),
            path: entry.path,
            name: metadata.name,
            description: metadata.description,
            warnings: metadata.warnings,
        });
    }
    inventory.commit = Some(commit.sha);
    cache.store(&inventory);
    Ok(inventory)
}

fn oversized(fallback: &str) -> Metadata {
    Metadata::unavailable(fallback, "file exceeds the 1 MiB metadata limit")
}

fn discover(
    client: &GitHubClient,
    repository: &Repository,
    root_sha: &str,
    walk_truncated: bool,
    progress: &mut impl FnMut(ScanProgress<'_>),
) -> Result<Vec<TreeEntry>, ScanError> {
    progress(ScanProgress::DiscoveringSkills);
    let recursive = client.tree(repository, root_sha, true)?;
    let mut candidates = Vec::new();
    if !recursive.truncated {
        validate_entries(&recursive.tree, true)?;
        candidates.extend(recursive.tree.into_iter().filter(is_skill));
    } else if !walk_truncated {
        return Err(ScanError::new("GitHub truncated this repository's file listing. Organization scans do not walk every directory of very large repositories; scan this repository individually."));
    } else {
        // A truncated recursive response cannot establish completeness. Walk
        // every directory from the pinned root, caching shared subtree objects.
        let mut pending = vec![(String::new(), root_sha.to_owned(), Vec::<String>::new())];
        let mut cache = HashMap::<String, Tree>::new();
        while let Some((prefix, sha, mut ancestors)) = pending.pop() {
            if ancestors.contains(&sha) {
                return Err(ScanError::new(
                    "GitHub returned a cyclic tree; the inventory is incomplete.",
                ));
            }
            ancestors.push(sha.clone());
            progress(ScanProgress::Directory(if prefix.is_empty() {
                "/"
            } else {
                &prefix
            }));
            if !cache.contains_key(&sha) {
                let tree = client.tree(repository, &sha, false)?;
                if tree.truncated {
                    return Err(ScanError::new("GitHub truncated a directory listing; the inventory is incomplete. Try a smaller repository."));
                }
                validate_entries(&tree.tree, false)?;
                cache.insert(sha.clone(), tree);
            }
            let tree = cache.get(&sha).expect("tree was cached");
            for entry in &tree.tree {
                let path = if prefix.is_empty() {
                    entry.path.clone()
                } else {
                    format!("{prefix}/{}", entry.path)
                };
                if entry.kind == "tree" {
                    pending.push((path, entry.sha.clone(), ancestors.clone()));
                } else if is_skill(entry) {
                    let mut candidate = entry.clone();
                    candidate.path = path;
                    candidates.push(candidate);
                }
            }
        }
    }
    candidates.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(candidates)
}

fn is_skill(entry: &TreeEntry) -> bool {
    is_regular_file(entry) && entry.path.rsplit('/').next() == Some("SKILL.md")
}

/// Symlinks and submodules are never read.
pub(crate) fn is_regular_file(entry: &TreeEntry) -> bool {
    entry.kind == "blob" && matches!(entry.mode.as_str(), "100644" | "100755")
}

pub(crate) fn validate_entries(entries: &[TreeEntry], recursive: bool) -> Result<(), ScanError> {
    let mut paths = HashSet::new();
    for entry in entries {
        validate_sha(&entry.sha)?;
        let valid_path = !entry.path.is_empty()
            && !entry.path.contains('\0')
            && entry
                .path
                .split('/')
                .all(|segment| !matches!(segment, "" | "." | ".."))
            && (recursive || !entry.path.contains('/'));
        let valid_mode = matches!(
            (entry.kind.as_str(), entry.mode.as_str()),
            ("tree", "040000") | ("blob", "100644" | "100755" | "120000") | ("commit", "160000")
        );
        if !valid_path || !valid_mode || !paths.insert(entry.path.as_str()) {
            return Err(ScanError::new("GitHub returned an invalid or duplicate tree entry; the inventory is incomplete. Retry the scan."));
        }
    }
    Ok(())
}
