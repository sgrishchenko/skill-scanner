use super::*;
use crate::test_support::TestDirectory;
use std::fs;

const COMMIT: &str = "1111111111111111111111111111111111111111";

fn repository(input: &str) -> Repository {
    input.parse().unwrap()
}

#[test]
fn persists_orders_replaces_unstars_and_restars_skills() {
    let directory = TestDirectory::new();
    let starred = StarredSkills::new(directory.0.clone());
    let review = repository("example/review-tools");
    starred
        .star_at(
            &review,
            "skills/test-plan/SKILL.md",
            "test-plan",
            COMMIT,
            100,
        )
        .unwrap();
    starred
        .star_at(&review, "SKILL.md", "review-tools", COMMIT, 200)
        .unwrap();
    starred
        .star_at(
            &repository("example/release-tools"),
            "skills/release-notes/SKILL.md",
            "release-notes",
            COMMIT,
            200,
        )
        .unwrap();
    // Reopen the store, as on a new invocation, and use another input form.
    let starred = StarredSkills::new(directory.0.clone());
    let upper = "2".repeat(40);
    starred
        .star_at(
            &repository("https://github.com/Example/Review-Tools.git/"),
            "skills/test-plan/SKILL.md",
            "Test plan",
            &upper,
            300,
        )
        .unwrap();
    let skills = starred.list().unwrap();
    assert_eq!(
        skills
            .iter()
            .map(|skill| (skill.repository.as_str(), skill.path.as_str()))
            .collect::<Vec<_>>(),
        [
            ("Example/Review-Tools", "skills/test-plan/SKILL.md"),
            ("example/release-tools", "skills/release-notes/SKILL.md"),
            ("example/review-tools", "SKILL.md"),
        ]
    );
    assert_eq!(
        skills[0],
        StarredSkill {
            repository: "Example/Review-Tools".into(),
            path: "skills/test-plan/SKILL.md".into(),
            name: "Test plan".into(),
            commit: upper.clone(),
            link: format!(
                "https://github.com/Example/Review-Tools/blob/{upper}/skills/test-plan/SKILL.md"
            ),
            starred_at: 300,
        }
    );
    starred
        .unstar(
            &repository("EXAMPLE/review-tools"),
            "skills/test-plan/SKILL.md",
        )
        .unwrap();
    starred
        .unstar(
            &repository("EXAMPLE/review-tools"),
            "skills/test-plan/SKILL.md",
        )
        .unwrap();
    assert_eq!(
        StarredSkills::new(directory.0.clone())
            .list()
            .unwrap()
            .len(),
        2
    );
    starred
        .star_at(
            &review,
            "skills/test-plan/SKILL.md",
            "test-plan",
            COMMIT,
            400,
        )
        .unwrap();
    assert_eq!(starred.list().unwrap()[0].path, "skills/test-plan/SKILL.md");
}

#[test]
fn paths_are_case_sensitive_and_cannot_escape_the_directory() {
    let directory = TestDirectory::new();
    let starred = StarredSkills::new(directory.0.clone());
    let repo = repository("a/b");
    for path in [
        "x/SKILL.md",
        "X/SKILL.md",
        "x/y/SKILL.md",
        ".hidden/SKILL.md",
    ] {
        starred.star_at(&repo, path, "skill", COMMIT, 1).unwrap();
    }
    starred
        .star_at(&repository("a/b--c"), "x/SKILL.md", "skill", COMMIT, 1)
        .unwrap();
    assert_eq!(starred.list().unwrap().len(), 5);
    let long = format!("{}/SKILL.md", "d".repeat(MAX_PATH_BYTES - 9));
    starred.star_at(&repo, &long, "skill", COMMIT, 1).unwrap();
    for path in [
        "",
        "SKILL.MD",
        "x/README.md",
        "/SKILL.md",
        "x//SKILL.md",
        "../SKILL.md",
        "x/./SKILL.md",
        "../../escape/SKILL.md",
        format!("d{long}").as_str(),
    ] {
        let error = starred
            .star_at(&repo, path, "skill", COMMIT, 1)
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{path}");
        let error = starred.unstar(&repo, path).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{path}");
    }
    for commit in [
        "",
        "abc",
        "A".repeat(40).as_str(),
        "g".repeat(40).as_str(),
        "1".repeat(41).as_str(),
    ] {
        assert!(starred
            .star_at(&repo, "SKILL.md", "skill", commit, 1)
            .is_err());
    }
    assert!(starred.star_at(&repo, "SKILL.md", "", COMMIT, 1).is_err());
    starred
        .star_at(&repo, "SKILL.md", "skill", &"a".repeat(64), 1)
        .unwrap();
    let invalid = Repository {
        owner: "..".into(),
        name: "escape".into(),
    };
    assert!(starred
        .star_at(&invalid, "SKILL.md", "skill", COMMIT, 1)
        .is_err());
    assert!(starred.unstar(&invalid, "SKILL.md").is_err());
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 7);
}

#[test]
fn long_names_are_shortened_and_escaped_entries_fit_the_size_limit() {
    let directory = TestDirectory::new();
    let starred = StarredSkills::new(directory.0.clone());
    let path = format!("{}/SKILL.md", "\u{1}".repeat(MAX_PATH_BYTES - 9));
    let name = "\u{2}".repeat(MAX_NAME_CHARS * 2);
    starred
        .star_at(&repository("a/b"), &path, &name, &"f".repeat(64), 1)
        .unwrap();
    let skills = starred.list().unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].path, path);
    assert_eq!(skills[0].name.chars().count(), MAX_NAME_CHARS);
}

#[test]
fn ignores_corrupt_incompatible_oversized_and_mismatched_entries() {
    let directory = TestDirectory::new();
    let starred = StarredSkills::new(directory.0.clone());
    starred
        .star_at(&repository("example/good"), "SKILL.md", "good", COMMIT, 1)
        .unwrap();
    let filename =
        |repo: &str, path: &str| StarredSkills::filename(&repository(repo), path).unwrap();
    let entry = |repo: &str, path: &str, name: &str, commit: &str, at: u64| {
        serde_json::to_vec(&serde_json::json!({
            "format_version": 1, "repository": repo, "path": path,
            "name": name, "commit": commit, "starred_at": at,
        }))
        .unwrap()
    };
    for (name, content) in [
        ("broken.json".to_owned(), b"{broken".to_vec()),
        ("large.json".to_owned(), vec![b' '; MAX_ENTRY_BYTES as usize + 1]),
        (
            filename("example/old", "SKILL.md"),
            br#"{"format_version":99,"repository":"example/old","path":"SKILL.md","name":"old","commit":"1111111111111111111111111111111111111111","starred_at":1}"#.to_vec(),
        ),
        (
            filename("example/other", "SKILL.md"),
            entry("example/good", "SKILL.md", "good", COMMIT, 1),
        ),
        (
            filename("example/good", "x/SKILL.md"),
            entry("example/good", "y/SKILL.md", "good", COMMIT, 1),
        ),
        (
            filename("example/name", "SKILL.md"),
            entry("example/name", "SKILL.md", "", COMMIT, 1),
        ),
        (
            filename("example/commit", "SKILL.md"),
            entry("example/commit", "SKILL.md", "commit", "main", 1),
        ),
        (
            filename("example/future", "SKILL.md"),
            entry("example/future", "SKILL.md", "future", COMMIT, u64::MAX),
        ),
        ("pending.tmp".to_owned(), b"{}".to_vec()),
    ] {
        fs::write(directory.0.join(name), content).unwrap();
    }
    let skills = starred.list().unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].repository, "example/good");
}

#[test]
fn disabled_missing_and_unavailable_storage_are_distinct() {
    let directory = TestDirectory::new();
    let repo = repository("example/skills");
    let missing = StarredSkills::new(directory.0.join("missing"));
    assert!(missing.list().unwrap().is_empty());
    missing.unstar(&repo, "SKILL.md").unwrap();
    assert!(!directory.0.join("missing").exists());
    let disabled = StarredSkills::disabled();
    disabled.star(&repo, "SKILL.md", "skill", COMMIT).unwrap();
    disabled.unstar(&repo, "SKILL.md").unwrap();
    assert!(!disabled.is_enabled());
    assert!(disabled.list().unwrap().is_empty());
    let file = directory.0.join("file");
    fs::write(&file, "not a directory").unwrap();
    let unavailable = StarredSkills::new(file.clone());
    assert!(unavailable.list().is_err());
    assert!(unavailable
        .star(&repo, "SKILL.md", "skill", COMMIT)
        .is_err());
    assert!(unavailable.unstar(&repo, "SKILL.md").is_err());
    assert_eq!(fs::read_to_string(file).unwrap(), "not a directory");
}

#[test]
fn concurrent_writers_keep_separate_stars_and_publish_valid_json() {
    let directory = TestDirectory::new();
    std::thread::scope(|scope| {
        for index in 0..8 {
            let starred = StarredSkills::new(directory.0.clone());
            scope.spawn(move || {
                let repo = repository("example/skills");
                for _ in 0..10 {
                    let path = format!("skills/{index}/SKILL.md");
                    starred.star(&repo, &path, "skill", COMMIT).unwrap();
                    starred.star(&repo, "SKILL.md", "shared", COMMIT).unwrap();
                    starred.list().expect("listing during concurrent writes");
                }
            });
        }
    });
    assert_eq!(
        StarredSkills::new(directory.0.clone())
            .list()
            .unwrap()
            .len(),
        9
    );
}

#[test]
fn starred_location_is_platform_specific_overridable_and_independent_of_history() {
    let root = env::temp_dir();
    assert_eq!(starred_root(|_| None), None);
    assert_eq!(starred_root(|_| Some(OsString::new())), None);
    assert_eq!(
        starred_root(
            |name| (name == "SKILL_SCANNER_STARRED_DIR").then(|| "relative-starred".into())
        ),
        Some("relative-starred".into())
    );
    let base = if cfg!(target_os = "macos") {
        root.join("Library/Application Support")
    } else {
        root.clone()
    };
    assert_eq!(
        starred_root(|name| match name {
            "HOME" | "LOCALAPPDATA" | "XDG_STATE_HOME" => Some(root.clone().into()),
            "SKILL_SCANNER_HISTORY_DIR" | "SKILL_SCANNER_CACHE_DIR" => Some("other".into()),
            _ => None,
        }),
        Some(base.join("skill-scanner/starred"))
    );
}

#[test]
fn path_hash_is_stable() {
    // Changing the hash would orphan existing stars.
    assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(
        StarredSkills::filename(&repository("Example/Skills"), "SKILL.md").unwrap(),
        format!(
            "owner-example_repo-skills_skill-{:016x}.json",
            fnv1a(b"SKILL.md")
        )
    );
}
