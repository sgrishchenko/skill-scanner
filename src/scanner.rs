use std::collections::{HashMap, HashSet};

use crate::{
    github::{validate_sha, BlobContent, GitHubClient, Tree, TreeEntry, MAX_SKILL_BYTES},
    metadata::{self, Metadata, MetadataWarning},
    repository::Repository,
    ScanError,
};

#[derive(Debug)]
pub struct Inventory {
    pub repository: Repository,
    pub commit: Option<String>,
    pub skills: Vec<Skill>,
}

#[derive(Debug)]
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
    mut progress: impl FnMut(ScanProgress<'_>),
) -> Result<Inventory, ScanError> {
    progress(ScanProgress::Repository(repository));
    let info = client.repository(repository)?;
    if info.private {
        return Err(ScanError(
            "Private repositories are unsupported; choose one public GitHub repository.".to_owned(),
        ));
    }
    if info.default_branch.is_empty() {
        return Err(ScanError(
            "GitHub did not identify a default branch; retry the scan.".to_owned(),
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
    let candidates = discover(client, repository, &commit.commit.tree.sha, &mut progress)?;
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
                .map_err(|error| ScanError(format!("{}: {error}", entry.path)))?
            {
                BlobContent::Bytes(bytes) => {
                    if entry.size.is_some_and(|size| size != bytes.len() as u64) {
                        return Err(ScanError(format!("{}: GitHub returned inconsistent file sizes; the inventory is incomplete. Retry the scan.", entry.path)));
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
    Ok(inventory)
}

fn oversized(fallback: &str) -> Metadata {
    Metadata::unavailable(fallback, "file exceeds the 1 MiB metadata limit")
}

fn discover(
    client: &GitHubClient,
    repository: &Repository,
    root_sha: &str,
    progress: &mut impl FnMut(ScanProgress<'_>),
) -> Result<Vec<TreeEntry>, ScanError> {
    progress(ScanProgress::DiscoveringSkills);
    let recursive = client.tree(repository, root_sha, true)?;
    let mut candidates = Vec::new();
    if !recursive.truncated {
        validate_entries(&recursive.tree, true)?;
        candidates.extend(recursive.tree.into_iter().filter(is_skill));
    } else {
        // A truncated recursive response cannot establish completeness. Walk
        // every directory from the pinned root, caching shared subtree objects.
        let mut pending = vec![(String::new(), root_sha.to_owned(), Vec::<String>::new())];
        let mut cache = HashMap::<String, Tree>::new();
        while let Some((prefix, sha, mut ancestors)) = pending.pop() {
            if ancestors.contains(&sha) {
                return Err(ScanError(
                    "GitHub returned a cyclic tree; the inventory is incomplete.".to_owned(),
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
                    return Err(ScanError("GitHub truncated a directory listing; the inventory is incomplete. Try a smaller repository.".to_owned()));
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
    entry.kind == "blob"
        && matches!(entry.mode.as_str(), "100644" | "100755")
        && entry.path.rsplit('/').next() == Some("SKILL.md")
}

fn validate_entries(entries: &[TreeEntry], recursive: bool) -> Result<(), ScanError> {
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
            return Err(ScanError("GitHub returned an invalid or duplicate tree entry; the inventory is incomplete. Retry the scan.".to_owned()));
        }
    }
    Ok(())
}
