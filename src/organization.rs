//! Scan every public, non-fork repository owned by one organization or user.

use std::collections::HashSet;

use crate::{
    cache::ScanCache,
    github::{GitHubClient, RepositoryInfo, OWNER_PAGE_SIZE},
    repository::{Owner, Repository},
    scanner::{self, ScanProgress, Skill},
    ScanError,
};

/// Stop runaway pagination; GitHub accounts this large are far beyond a local scan.
const MAX_PAGES: usize = 1000;

#[derive(Debug)]
pub struct OrganizationInventory {
    /// GitHub's spelling of the owner when any repository was listed.
    pub owner: Owner,
    /// Every listed public repository except forks, ordered case-insensitively.
    pub repositories: Vec<RepositorySummary>,
    pub skipped_forks: usize,
    /// Skills from completed repositories. Each path starts with `owner/repository/`
    /// so ordering, grouping, and search distinguish repositories.
    pub skills: Vec<Skill>,
}

#[derive(Debug, serde::Serialize)]
pub struct RepositorySummary {
    pub repository: String,
    /// Null for an empty repository or a failed scan.
    pub commit: Option<String>,
    pub skill_count: usize,
    /// Present only when this repository could not be scanned completely.
    pub error: Option<String>,
}

impl OrganizationInventory {
    pub fn failed_repositories(&self) -> usize {
        self.repositories
            .iter()
            .filter(|repository| repository.error.is_some())
            .count()
    }
}

#[derive(Debug)]
pub enum OrganizationProgress<'a> {
    Listing {
        owner: &'a Owner,
        page: usize,
    },
    Repository {
        repository: &'a Repository,
        current: usize,
        total: usize,
    },
    /// Progress within the repository most recently reported.
    Scan(ScanProgress<'a>),
}

/// List the owner's public repositories, then scan them one at a time, as
/// GitHub recommends for API clients. Each repository uses the shared cache.
/// A repository-specific failure is recorded and the scan continues; credential,
/// rate-limit, and connection failures stop the scan because they affect every
/// later repository. Organization scans do not update recent repositories.
pub fn scan_organization(
    client: &GitHubClient,
    owner: &Owner,
    cache: &ScanCache,
    mut progress: impl FnMut(OrganizationProgress<'_>),
) -> Result<OrganizationInventory, ScanError> {
    let mut listed = Vec::new();
    let mut seen = HashSet::new();
    let mut skipped_forks = 0;
    let mut canonical = None;
    for page in 1.. {
        if page > MAX_PAGES {
            return Err(ScanError::new(format!("{owner} has more than {} public repositories; scan individual repositories instead.", MAX_PAGES * OWNER_PAGE_SIZE)));
        }
        progress(OrganizationProgress::Listing { owner, page });
        let entries = client.owner_repositories(owner, page)?;
        let last = entries.len() < OWNER_PAGE_SIZE;
        for entry in entries {
            let repository = Repository {
                owner: entry.owner.login.clone(),
                name: entry.name.clone(),
            };
            // Validate GitHub's data like user input; the suffix preserves names
            // that themselves end in .git.
            if !entry.owner.login.eq_ignore_ascii_case(&owner.login)
                || format!("{repository}.git").parse::<Repository>().as_ref() != Ok(&repository)
            {
                return Err(ScanError::new("GitHub returned an invalid repository list; the organization inventory is incomplete. Retry the scan."));
            }
            if canonical.is_none() {
                canonical = Some(entry.owner.login.clone());
            }
            // Pages can shift while repositories are created or renamed.
            if entry.private || !seen.insert(entry.name.to_ascii_lowercase()) {
                continue;
            }
            if entry.fork {
                skipped_forks += 1;
                continue;
            }
            listed.push((repository, entry));
        }
        if last {
            break;
        }
    }
    listed.sort_by(|(left, _), (right, _)| {
        left.name
            .to_ascii_lowercase()
            .cmp(&right.name.to_ascii_lowercase())
            .then_with(|| left.name.cmp(&right.name))
    });

    let total = listed.len();
    let mut repositories = Vec::with_capacity(total);
    let mut skills = Vec::new();
    for (index, (repository, entry)) in listed.into_iter().enumerate() {
        progress(OrganizationProgress::Repository {
            repository: &repository,
            current: index + 1,
            total,
        });
        let result = if entry.disabled {
            Err(ScanError::new(
                "GitHub has disabled this repository, so it cannot be scanned.",
            ))
        } else {
            let info = RepositoryInfo {
                private: false,
                default_branch: entry.default_branch,
            };
            // Very large repositories would need a request per directory.
            scanner::scan_snapshot(client, &repository, &info, cache, false, |inner| {
                progress(OrganizationProgress::Scan(inner))
            })
        };
        match result {
            Ok(inventory) => {
                repositories.push(RepositorySummary {
                    repository: repository.to_string(),
                    commit: inventory.commit,
                    skill_count: inventory.skills.len(),
                    error: None,
                });
                skills.extend(inventory.skills.into_iter().map(|mut skill| {
                    skill.path = format!("{repository}/{}", skill.path);
                    skill
                }));
            }
            Err(error) if error.is_service() => {
                return Err(error.context(&format!(
                    "Stopped at {repository} (repository {} of {total})",
                    index + 1
                )));
            }
            Err(error) => repositories.push(RepositorySummary {
                repository: repository.to_string(),
                commit: None,
                skill_count: 0,
                error: Some(error.to_string()),
            }),
        }
    }
    skills.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(OrganizationInventory {
        owner: Owner {
            login: canonical.unwrap_or_else(|| owner.login.clone()),
        },
        repositories,
        skipped_forks,
        skills,
    })
}
