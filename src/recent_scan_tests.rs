use super::*;
use crate::{cache::ScanCache, recent::RecentRepositories};

#[test]
fn successful_empty_cached_and_warning_scans_are_remembered_but_failures_are_not() {
    let directory = TestDirectory::new();
    let recent = RecentRepositories::new(directory.0.join("recent"));
    let cache = ScanCache::new(directory.0.join("cache"));
    let mut replies = snapshot();
    replies.extend([
        tree(ROOT, true, false, vec![skill("SKILL.md", A)]),
        blob(A, include_bytes!("../tests/fixtures/malformed.md")),
    ]);
    for replies in [
        replies,
        snapshot(),
        vec![
            repository(),
            Reply::json(
                "/repos/example/skills/commits/main",
                409,
                json!({"message":"Git Repository is empty."}),
            ),
        ],
    ] {
        let server = Server::start(replies);
        let inventory = scanner::scan_with_storage(
            &server.client(),
            &"example/skills".parse().unwrap(),
            &cache,
            &recent,
            |_| {},
        )
        .unwrap();
        let entries = recent.list().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].skill_count, inventory.skills.len());
        server.finish();
    }
    let before = recent.list().unwrap();
    let server = Server::start(vec![Reply::json("/repos/example/skills", 404, json!({}))]);
    assert!(scanner::scan_with_storage(
        &server.client(),
        &"example/skills".parse().unwrap(),
        &cache,
        &recent,
        |_| {}
    )
    .is_err());
    assert_eq!(recent.list().unwrap(), before);
    server.finish();
    recent.remove(&"example/skills".parse().unwrap()).unwrap();
    assert!(cache
        .load(&"example/skills".parse().unwrap(), COMMIT)
        .is_some());
}

#[test]
fn history_write_failure_warns_without_failing_the_scan() {
    let directory = TestDirectory::new();
    let path = directory.0.join("unavailable");
    std::fs::write(&path, "file").unwrap();
    let mut replies = snapshot();
    replies.push(tree(ROOT, true, false, vec![]));
    let server = Server::start(replies);
    let mut warning = false;
    let result = scanner::scan_with_storage(
        &server.client(),
        &"example/skills".parse().unwrap(),
        &ScanCache::disabled(),
        &RecentRepositories::new(path),
        |event| {
            warning |= matches!(event, scanner::ScanProgress::HistoryWarning(_));
        },
    );
    assert!(result.is_ok());
    assert!(warning);
    server.finish();
}
