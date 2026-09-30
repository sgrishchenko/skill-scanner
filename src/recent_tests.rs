use super::*;
use crate::test_support::TestDirectory;

fn inventory(repository: &str) -> Inventory {
    Inventory {
        repository: repository.parse().unwrap(),
        commit: None,
        skills: Vec::new(),
    }
}

#[test]
fn persists_orders_deduplicates_removes_and_readds_repositories() {
    let directory = TestDirectory::new();
    let recent = RecentRepositories::new(directory.0.clone());
    recent.record_at(&inventory("example/first"), 100).unwrap();
    recent.record_at(&inventory("example/second"), 200).unwrap();
    // Reopen the store, as on a new invocation, and use another input form.
    let recent = RecentRepositories::new(directory.0.clone());
    recent
        .record_at(&inventory("https://github.com/Example/First.git/"), 300)
        .unwrap();
    let entries = recent.list().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].repository, "Example/First");
    assert_eq!(entries[0].scanned_at, 300);
    assert_eq!(entries[0].skill_count, 0);
    assert_eq!(entries[1].repository, "example/second");
    recent.remove(&"EXAMPLE/FIRST".parse().unwrap()).unwrap();
    recent.remove(&"EXAMPLE/FIRST".parse().unwrap()).unwrap();
    assert_eq!(
        RecentRepositories::new(directory.0.clone())
            .list()
            .unwrap()
            .len(),
        1
    );
    recent.record_at(&inventory("example/first"), 400).unwrap();
    assert_eq!(recent.list().unwrap()[0].repository, "example/first");
}

#[test]
fn distinct_repositories_cannot_collide_or_escape_the_directory() {
    let directory = TestDirectory::new();
    let recent = RecentRepositories::new(directory.0.clone());
    for name in ["a/b--c", "a--b/c", "a/CON", "a/b.git.git", "a/_repo-c"] {
        recent.record_at(&inventory(name), 100).unwrap();
    }
    assert_eq!(recent.list().unwrap().len(), 5);
    let invalid = Repository {
        owner: "..".into(),
        name: "escape".into(),
    };
    assert!(recent.remove(&invalid).is_err());
}

#[test]
fn ignores_corrupt_incompatible_oversized_and_mismatched_entries() {
    let directory = TestDirectory::new();
    let recent = RecentRepositories::new(directory.0.clone());
    recent.record_at(&inventory("example/good"), 100).unwrap();
    for (name, content) in [
        ("broken.json", b"{broken".to_vec()),
        ("large.json", vec![b' '; MAX_ENTRY_BYTES as usize + 1]),
        ("owner-example_repo-old.json", br#"{"format_version":99,"repository":"example/old","scanned_at":1,"skill_count":0}"#.to_vec()),
        ("owner-example_repo-other.json", br#"{"format_version":1,"repository":"example/good","scanned_at":1,"skill_count":0}"#.to_vec()),
        ("owner-example_repo-bad.json", br#"{"format_version":1,"repository":"<script>/bad","scanned_at":1,"skill_count":0}"#.to_vec()),
        ("owner-example_repo-future.json", br#"{"format_version":1,"repository":"example/future","scanned_at":18446744073709551615,"skill_count":0}"#.to_vec()),
        ("pending.tmp", b"{}".to_vec()),
    ] {
        fs::write(directory.0.join(name), content).unwrap();
    }
    assert_eq!(recent.list().unwrap().len(), 1);
    assert_eq!(recent.list().unwrap()[0].repository, "example/good");
}

#[test]
fn disabled_missing_and_unavailable_storage_are_distinct() {
    let directory = TestDirectory::new();
    let missing = RecentRepositories::new(directory.0.join("missing"));
    assert!(missing.list().unwrap().is_empty());
    assert!(missing.remove(&"example/skills".parse().unwrap()).is_ok());
    assert!(!directory.0.join("missing").exists());
    let disabled = RecentRepositories::disabled();
    disabled.record(&inventory("example/skills")).unwrap();
    assert!(!disabled.is_enabled());
    assert!(disabled.list().unwrap().is_empty());
    let file = directory.0.join("file");
    fs::write(&file, "not a directory").unwrap();
    let unavailable = RecentRepositories::new(file);
    assert!(unavailable.list().is_err());
    assert!(unavailable.record(&inventory("example/skills")).is_err());
    assert!(unavailable
        .remove(&"example/skills".parse().unwrap())
        .is_err());
}

#[test]
fn concurrent_writers_keep_separate_entries_and_publish_valid_json() {
    let directory = TestDirectory::new();
    std::thread::scope(|scope| {
        for index in 0..8 {
            let recent = RecentRepositories::new(directory.0.clone());
            scope.spawn(move || {
                for _ in 0..10 {
                    recent
                        .record(&inventory(&format!("example/repo-{index}")))
                        .unwrap();
                    recent.record(&inventory("example/shared")).unwrap();
                    assert!(recent.list().is_ok());
                }
            });
        }
    });
    assert_eq!(
        RecentRepositories::new(directory.0.clone())
            .list()
            .unwrap()
            .len(),
        9
    );
}

#[test]
fn history_location_is_platform_specific_overridable_and_independent_of_cache() {
    let root = env::temp_dir();
    assert_eq!(history_root(|_| None), None);
    assert_eq!(history_root(|_| Some(OsString::new())), None);
    assert_eq!(
        history_root(
            |name| (name == "SKILL_SCANNER_HISTORY_DIR").then(|| "relative-history".into())
        ),
        Some("relative-history".into())
    );
    let base = if cfg!(target_os = "macos") {
        root.join("Library/Application Support")
    } else {
        root.clone()
    };
    assert_eq!(
        history_root(
            |name| matches!(name, "HOME" | "LOCALAPPDATA" | "XDG_STATE_HOME")
                .then(|| root.clone().into())
        ),
        Some(base.join("skill-scanner/recent"))
    );
    if !cfg!(any(target_os = "macos", target_os = "windows")) {
        assert_eq!(
            history_root(|name| match name {
                "HOME" => Some(root.clone().into()),
                "XDG_STATE_HOME" => Some("relative".into()),
                "SKILL_SCANNER_CACHE_DIR" => Some("cache".into()),
                _ => None,
            }),
            Some(root.join(".local/state/skill-scanner/recent"))
        );
    }
}
