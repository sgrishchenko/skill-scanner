use super::*;
use crate::{cache::ScanCache, scanner::Inventory};
use std::fs;

fn analysis() -> Vec<Reply> {
    let mut replies = snapshot();
    replies.extend([
        tree(
            ROOT,
            true,
            false,
            vec![
                skill("b/SKILL.md", A),
                skill("SKILL.md", B),
                skill("a/SKILL.md", A),
            ],
        ),
        blob(B, include_bytes!("../tests/fixtures/malformed.md")),
        blob(A, VALID),
        blob(A, VALID),
    ]);
    replies
}

fn scan_cached(server: &Server, cache: &ScanCache) -> Inventory {
    scanner::scan_with_cache(
        &server.client(),
        &"example/skills".parse().unwrap(),
        cache,
        |_| {},
    )
    .unwrap()
}

fn snapshot_at(branch: &str, commit: &str) -> Vec<Reply> {
    vec![
        Reply::json(
            "/repos/example/skills",
            200,
            json!({"private": false, "default_branch": branch}),
        ),
        Reply::json(
            format!("/repos/example/skills/commits/{branch}"),
            200,
            json!({"sha": commit, "commit": {"tree": {"sha": ROOT}}}),
        ),
    ]
}

#[test]
fn reuses_persisted_analysis_with_warnings_and_identical_reports_after_revalidation() {
    let directory = TestDirectory::new();
    let server = Server::start(analysis());
    let fresh = scan_cached(&server, &ScanCache::new(directory.0.clone()));
    assert_eq!(server.finish().len(), 6);

    // New cache and client instances model another process using the same disk.
    let server = Server::start(snapshot());
    let mut progress = Vec::new();
    let cached = scanner::scan_with_cache(
        &server.client(),
        &"https://github.com/example/skills.git/".parse().unwrap(),
        &ScanCache::new(directory.0.clone()),
        |event| report::write_progress(&mut progress, event).unwrap(),
    )
    .unwrap();
    assert_eq!(server.finish().len(), 2);
    assert_eq!(
        serde_json::to_value(&cached).unwrap(),
        serde_json::to_value(&fresh).unwrap()
    );
    let mut fresh_report = Vec::new();
    let mut cached_report = Vec::new();
    report::write_report(&mut fresh_report, &fresh).unwrap();
    report::write_report(&mut cached_report, &cached).unwrap();
    assert_eq!(fresh_report, cached_report);
    let mut fresh_warnings = Vec::new();
    let mut cached_warnings = Vec::new();
    report::write_warnings(&mut fresh_warnings, &fresh).unwrap();
    report::write_warnings(&mut cached_warnings, &cached).unwrap();
    assert_eq!(fresh_warnings, cached_warnings);
    assert!(!fresh_warnings.is_empty());
    let progress = String::from_utf8(progress).unwrap();
    assert_eq!(progress, format!("Resolving repository: example/skills\nResolving default branch: main\nUsing cached analysis for commit: {COMMIT}\n"));
}

#[test]
fn shares_cache_across_input_case_and_regenerates_links_and_root_fallback() {
    let directory = TestDirectory::new();
    let cache = ScanCache::new(directory.0.clone());
    let server = Server::start(analysis());
    scan_cached(&server, &cache);
    server.finish();
    let mut replies = snapshot();
    for reply in &mut replies {
        reply.path = reply.path.replace("example/skills", "Example/Skills");
    }
    let server = Server::start(replies);
    let inventory = scanner::scan_with_cache(
        &server.client(),
        &"Example/Skills".parse().unwrap(),
        &cache,
        |_| {},
    )
    .unwrap();
    assert_eq!(inventory.repository.to_string(), "Example/Skills");
    assert_eq!(inventory.skills[0].name, "Skills");
    assert!(inventory.skills.iter().all(|skill| skill
        .link
        .starts_with("https://github.com/Example/Skills/blob/")));
    assert_eq!(server.finish().len(), 2);
}

#[test]
fn changed_commit_or_default_branch_triggers_new_analysis_and_replaces_the_entry() {
    let directory = TestDirectory::new();
    let cache = ScanCache::new(directory.0.clone());
    let server = Server::start(analysis());
    scan_cached(&server, &cache);
    server.finish();

    let mut replies = snapshot_at("next", D);
    replies.extend([
        tree(ROOT, true, false, vec![skill("new/SKILL.md", A)]),
        blob(A, VALID),
    ]);
    let server = Server::start(replies);
    let inventory = scan_cached(&server, &cache);
    assert_eq!(inventory.commit.as_deref(), Some(D));
    assert_eq!(inventory.skills.len(), 1);
    assert_eq!(inventory.skills[0].path, "new/SKILL.md");
    assert!(inventory.skills[0].link.contains(D));
    assert_eq!(server.finish().len(), 4);
    assert!(cache.load(&inventory.repository, COMMIT).is_none());
    let server = Server::start(snapshot_at("next", D));
    assert_eq!(scan_cached(&server, &cache).skills.len(), 1);
    assert_eq!(server.finish().len(), 2);
}

#[test]
fn cached_analysis_never_bypasses_access_or_commit_resolution_failures() {
    let directory = TestDirectory::new();
    let cache = ScanCache::new(directory.0.clone());
    let server = Server::start(analysis());
    scan_cached(&server, &cache);
    server.finish();
    for replies in [
        vec![Reply::json("/repos/example/skills", 404, json!({}))],
        vec![Reply::json("/repos/example/skills", 403, json!({}))],
        vec![Reply::json(
            "/repos/example/skills",
            200,
            json!({"private": true, "default_branch": "main"}),
        )],
        vec![
            repository(),
            Reply::json("/repos/example/skills/commits/main", 404, json!({})),
        ],
        vec![
            repository(),
            Reply::json("/repos/example/skills/commits/main", 200, json!({})),
        ],
    ] {
        let server = Server::start(replies);
        let mut hit = false;
        let result = scanner::scan_with_cache(
            &server.client(),
            &"example/skills".parse().unwrap(),
            &cache,
            |event| hit |= matches!(event, scanner::ScanProgress::Cached(_)),
        );
        assert!(result.is_err());
        assert!(!hit);
        server.finish();
    }
}

#[test]
fn incomplete_refresh_does_not_publish_or_replace_a_complete_entry() {
    let directory = TestDirectory::new();
    let cache = ScanCache::new(directory.0.clone());
    let server = Server::start(analysis());
    let original = scan_cached(&server, &cache);
    server.finish();
    for _ in 0..2 {
        let mut replies = snapshot_at("main", D);
        replies.extend([
            tree(
                ROOT,
                true,
                false,
                vec![skill("a/SKILL.md", A), skill("b/SKILL.md", B)],
            ),
            blob(A, VALID),
            Reply::json(
                format!("/repos/example/skills/git/blobs/{B}"),
                404,
                json!({}),
            ),
        ]);
        let server = Server::start(replies);
        assert!(
            scanner::scan_with_cache(&server.client(), &original.repository, &cache, |_| {})
                .is_err()
        );
        assert_eq!(server.finish().len(), 5);
        assert!(cache.load(&original.repository, D).is_none());
        assert!(cache.load(&original.repository, COMMIT).is_some());
    }
}

#[test]
fn invalid_or_oversized_cache_entries_are_replaced_by_a_fresh_scan() {
    let directory = TestDirectory::new();
    let cache = ScanCache::new(directory.0.clone());
    let server = Server::start(analysis());
    let original = scan_cached(&server, &cache);
    server.finish();
    let path = directory.0.join("owner-example/repo-skills.json");
    let valid: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let mut corrupt = vec![b"{truncated".to_vec()];
    for (pointer, replacement) in [
        ("/format_version", json!(0)),
        ("/scanner_version", json!("old-version")),
        ("/inventory/repository/name", json!("another-repository")),
        ("/inventory/commit", json!(D)),
        ("/inventory/skills", Value::Null),
        ("/inventory/skills/0/path", json!("../SKILL.md")),
        ("/inventory/skills/0/warnings/0/field", json!("unknown")),
        ("/inventory/skills/1/path", json!("SKILL.md")),
    ] {
        let mut value = valid.clone();
        *value.pointer_mut(pointer).unwrap() = replacement;
        corrupt.push(serde_json::to_vec(&value).unwrap());
    }
    for bytes in corrupt {
        fs::write(&path, bytes).unwrap();
        let server = Server::start(analysis());
        let refreshed = scan_cached(&server, &cache);
        assert_eq!(
            serde_json::to_value(&refreshed).unwrap(),
            serde_json::to_value(&original).unwrap()
        );
        assert_eq!(server.finish().len(), 6);
    }
    fs::File::create(&path)
        .unwrap()
        .set_len(32 * 1024 * 1024 + 1)
        .unwrap();
    let server = Server::start(analysis());
    assert_eq!(scan_cached(&server, &cache).skills.len(), 3);
    assert_eq!(server.finish().len(), 6);
}

#[test]
fn unavailable_or_disabled_cache_does_not_fail_scans() {
    let directory = TestDirectory::new();
    let file = directory.0.join("not-a-directory");
    fs::write(&file, b"occupied").unwrap();
    for cache in [ScanCache::new(file.clone()), ScanCache::disabled()] {
        for _ in 0..2 {
            let server = Server::start(analysis());
            assert_eq!(scan_cached(&server, &cache).skills.len(), 3);
            assert_eq!(server.finish().len(), 6);
        }
    }
    assert_eq!(fs::read(&file).unwrap(), b"occupied");
}

#[test]
fn caches_zero_skills_but_rechecks_empty_repositories_without_a_commit() {
    let directory = TestDirectory::new();
    let cache = ScanCache::new(directory.0.clone());
    let mut replies = snapshot();
    replies.push(tree(ROOT, true, false, vec![]));
    replies.extend(snapshot());
    let server = Server::start(replies);
    for _ in 0..2 {
        let inventory = scan_cached(&server, &cache);
        assert!(inventory.skills.is_empty());
        assert_eq!(inventory.commit.as_deref(), Some(COMMIT));
    }
    assert_eq!(server.finish().len(), 5);
    for _ in 0..2 {
        let server = Server::start(vec![
            repository(),
            Reply::json(
                "/repos/example/skills/commits/main",
                409,
                json!({"message":"Git Repository is empty."}),
            ),
        ]);
        let inventory = scan_cached(&server, &cache);
        assert!(inventory.commit.is_none());
        assert!(inventory.skills.is_empty());
        assert_eq!(server.finish().len(), 2);
    }
}
