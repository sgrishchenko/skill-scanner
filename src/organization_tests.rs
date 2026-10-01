use super::*;
use crate::{
    cache::ScanCache,
    organization::{self, OrganizationInventory},
    repository::Owner,
};

pub(super) fn listing(page: usize, entries: Vec<Value>) -> Reply {
    Reply::json(
        format!("/users/example/repos?type=owner&sort=full_name&per_page=100&page={page}"),
        200,
        Value::Array(entries),
    )
}

pub(super) fn listed(name: &str) -> Value {
    json!({
        "name": name, "owner": { "login": "example" }, "private": false,
        "fork": false, "archived": false, "disabled": false, "default_branch": "main"
    })
}

fn with(mut entry: Value, field: &str, value: Value) -> Value {
    entry[field] = value;
    entry
}

pub(super) fn commit(repository: &str) -> Reply {
    Reply::json(
        format!("/repos/example/{repository}/commits/main"),
        200,
        json!({ "sha": COMMIT, "commit": { "tree": { "sha": ROOT } } }),
    )
}

pub(super) fn recursive_tree(repository: &str, truncated: bool, entries: Vec<Value>) -> Reply {
    Reply::json(
        format!("/repos/example/{repository}/git/trees/{ROOT}?recursive=1"),
        200,
        json!({ "sha": ROOT, "truncated": truncated, "tree": entries }),
    )
}

pub(super) fn repository_blob(repository: &str, sha: &str, content: &[u8]) -> Reply {
    let mut reply = blob(sha, content);
    reply.path = format!("/repos/example/{repository}/git/blobs/{sha}");
    reply
}

fn owner() -> Owner {
    "example".parse().unwrap()
}

fn scan(server: &Server, cache: &ScanCache) -> (Result<OrganizationInventory, String>, String) {
    let mut progress = Vec::new();
    let result = organization::scan_organization(&server.client(), &owner(), cache, |event| {
        report::write_organization_progress(&mut progress, event).unwrap();
    })
    .map_err(|error| error.to_string());
    (result, String::from_utf8(progress).unwrap())
}

#[test]
fn scans_public_non_fork_repositories_in_order_and_reports_repository_failures() {
    let renamed = b"---\nname: Code Review\ndescription: Another reviewer.\n---\n";
    let server = Server::start(vec![
        listing(
            1,
            vec![
                listed("zeta"),
                with(listed("fork-of-tools"), "fork", json!(true)),
                listed("broken"),
                with(listed("hidden"), "private", json!(true)),
                listed("Alpha"),
                with(listed("off"), "disabled", json!(true)),
                listed("empty"),
                // A repository repeated by shifting pages is scanned once.
                listed("zeta"),
            ],
        ),
        commit("Alpha"),
        recursive_tree(
            "Alpha",
            false,
            vec![skill("b/SKILL.md", A), skill("SKILL.md", B)],
        ),
        repository_blob("Alpha", B, include_bytes!("../tests/fixtures/missing.md")),
        repository_blob("Alpha", A, VALID),
        commit("broken"),
        // Organization scans do not walk every directory of huge repositories.
        recursive_tree("broken", true, vec![]),
        Reply::json(
            "/repos/example/empty/commits/main",
            409,
            json!({ "message": "Git Repository is empty." }),
        ),
        commit("zeta"),
        recursive_tree("zeta", false, vec![skill("review/SKILL.md", C)]),
        repository_blob("zeta", C, renamed),
    ]);
    let (result, progress) = scan(&server, &ScanCache::disabled());
    server.finish();
    let inventory = result.unwrap();
    assert!(progress.starts_with(
        "Listing public repositories: example (page 1)\nScanning repository [1/5]: example/Alpha\n  Resolving default branch: main\n  Discovering SKILL.md files: /\n  Scanning skill [1/2]: SKILL.md\n"
    ));
    assert!(progress.contains(
        "Scanning repository [4/5]: example/off\nScanning repository [5/5]: example/zeta\n"
    ));
    assert_eq!(inventory.owner.login, "example");
    assert_eq!(inventory.skipped_forks, 1);
    assert_eq!(inventory.failed_repositories(), 2);
    let summaries: Vec<_> = inventory
        .repositories
        .iter()
        .map(|summary| {
            (
                summary.repository.as_str(),
                summary.commit.as_deref(),
                summary.skill_count,
                summary.error.is_some(),
            )
        })
        .collect();
    assert_eq!(
        summaries,
        [
            ("example/Alpha", Some(COMMIT), 2, false),
            ("example/broken", None, 0, true),
            ("example/empty", None, 0, false),
            ("example/off", None, 0, true),
            ("example/zeta", Some(COMMIT), 1, false),
        ]
    );
    assert!(inventory.repositories[1]
        .error
        .as_deref()
        .unwrap()
        .contains("scan this repository individually"));
    assert_eq!(
        inventory
            .skills
            .iter()
            .map(|skill| (skill.path.as_str(), skill.name.as_str()))
            .collect::<Vec<_>>(),
        [
            ("example/Alpha/SKILL.md", "Alpha"),
            ("example/Alpha/b/SKILL.md", "code-review"),
            ("example/zeta/review/SKILL.md", "Code Review"),
        ]
    );
    assert_eq!(
        inventory.skills[2].link,
        format!("https://github.com/example/zeta/blob/{COMMIT}/review/SKILL.md")
    );

    // Similarity spans repositories, and failures are listed before skills.
    let mut output = Vec::new();
    let mut warnings = Vec::new();
    report::write_organization_report(&mut output, &inventory).unwrap();
    report::write_skill_warnings(&mut warnings, &inventory.skills).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.starts_with("Organization: example\nRepositories scanned: 5\nRepositories with skills: 2\nRepositories without skills: 1\nFailed repositories: 2\nForks skipped: 1\n\nIncomplete: these repositories could not be scanned, so their skills are missing:\n  example/broken: GitHub truncated"));
    assert!(output.contains("  example/off: GitHub has disabled this repository"));
    assert!(output.contains(&format!(
        "\nRepositories with skills:\n  example/Alpha: 2 skills at {COMMIT}\n  example/zeta: 1 skill at {COMMIT}\n\nSkills found: 3\nSimilar groups: 1\n"
    )));
    assert!(output.contains(
        "  code-review: 2 skills (66.7% of scan), 0 with warnings\n    example/Alpha/b/SKILL.md\n    example/zeta/review/SKILL.md\n"
    ));
    assert!(output.contains("  Path: example/Alpha/SKILL.md\n"));
    assert!(String::from_utf8(warnings)
        .unwrap()
        .starts_with("warning: example/Alpha/SKILL.md: name:"));
}

#[test]
fn follows_full_pages_and_stops_on_service_failures() {
    let forks = (0..100)
        .map(|index| with(listed(&format!("fork-{index:03}")), "fork", json!(true)))
        .collect();
    let server = Server::start(vec![
        listing(1, forks),
        listing(2, vec![listed("tools"), listed("zeta")]),
        Reply::json("/repos/example/tools/commits/main", 403, json!({})),
    ]);
    let (result, progress) = scan(&server, &ScanCache::disabled());
    server.finish();
    let error = result.unwrap_err();
    assert!(
        error.starts_with("Stopped at example/tools (repository 1 of 2): GitHub denied access or its API rate limit was reached"),
        "{error}"
    );
    assert!(error.contains("GITHUB_TOKEN"));
    assert!(progress.contains("Listing public repositories: example (page 2)\n"));
    assert!(!progress.contains("example/zeta"));
}

#[test]
fn listing_failures_and_invalid_listings_return_no_inventory() {
    let server = Server::start(vec![Reply::json(
        "/users/example/repos?type=owner&sort=full_name&per_page=100&page=1",
        404,
        json!({ "message": "Not Found" }),
    )]);
    let (result, _) = scan(&server, &ScanCache::disabled());
    server.finish();
    assert!(result
        .unwrap_err()
        .contains("organization or user is inaccessible"));

    for entry in [
        with(listed("tools"), "owner", json!({ "login": "other" })),
        with(listed("tools"), "name", json!("../tools")),
    ] {
        let server = Server::start(vec![listing(1, vec![entry])]);
        let (result, _) = scan(&server, &ScanCache::disabled());
        server.finish();
        assert!(result.unwrap_err().contains("invalid repository list"));
    }

    let server = Server::start(vec![listing(1, vec![])]);
    let (result, _) = scan(&server, &ScanCache::disabled());
    server.finish();
    let inventory = result.unwrap();
    assert!(inventory.repositories.is_empty());
    assert!(inventory.skills.is_empty());
    let mut output = Vec::new();
    report::write_organization_report(&mut output, &inventory).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("No public repositories to scan.\n"));
    assert!(output.ends_with("Skills found: 0\nSimilar groups: 0\nSkills in similar groups: 0 (0.0%)\nStandalone skills: 0\nLargest similar group: 0\nSkills with warnings: 0\nNo SKILL.md files found.\n"));
}

#[test]
fn reuses_cached_repository_analyses_after_revalidating_commits() {
    let directory = TestDirectory::new();
    let cache = ScanCache::new(directory.0.clone());
    let server = Server::start(vec![
        listing(1, vec![listed("tools")]),
        commit("tools"),
        recursive_tree("tools", false, vec![skill("review/SKILL.md", A)]),
        repository_blob("tools", A, VALID),
    ]);
    let (fresh, _) = scan(&server, &cache);
    server.finish();

    let server = Server::start(vec![listing(1, vec![listed("tools")]), commit("tools")]);
    let (cached, progress) = scan(&server, &ScanCache::new(directory.0.clone()));
    assert_eq!(server.finish().len(), 2);
    assert!(progress.contains(&format!("  Using cached analysis for commit: {COMMIT}\n")));
    let (fresh, cached) = (fresh.unwrap(), cached.unwrap());
    assert_eq!(cached.skills.len(), 1);
    assert_eq!(cached.skills[0].path, fresh.skills[0].path);
    assert_eq!(cached.skills[0].link, fresh.skills[0].link);
}
