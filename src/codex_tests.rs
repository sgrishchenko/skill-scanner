use super::*;
use crate::test_support::TestDirectory;

const COMMIT: &str = "1111111111111111111111111111111111111111";

fn repository(input: &str) -> Repository {
    input.parse().unwrap()
}

/// Write an installation record as an earlier install would have.
fn installed(root: &Path, folder: &str, repository: &str, path: &str, installed_at: u64) {
    fs::create_dir_all(root.join(folder)).unwrap();
    fs::write(root.join(folder).join("SKILL.md"), "---\nname: x\n---\n").unwrap();
    fs::write(
        root.join(folder).join(MARKER),
        serde_json::to_vec(&Marker {
            format_version: FORMAT_VERSION,
            repository: repository.to_owned(),
            path: path.to_owned(),
            commit: COMMIT.to_owned(),
            installed_at,
        })
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn folder_names_come_from_the_skill_directory_or_the_repository() {
    let skills = repository("example/skills");
    assert_eq!(
        install_name(&skills, "skills/code-review/SKILL.md").unwrap(),
        "code-review"
    );
    assert_eq!(
        install_name(&skills, "a/b/Release_Notes.v2/SKILL.md").unwrap(),
        "Release_Notes.v2"
    );
    assert_eq!(install_name(&skills, "SKILL.md").unwrap(), "skills");
    assert!(install_name(&repository("example/.github"), "SKILL.md").is_err());
    let long = format!("{}/SKILL.md", "a".repeat(MAX_NAME_BYTES + 1));
    for path in [
        "",
        "README.md",
        "skill.md",
        "../SKILL.md",
        "a//SKILL.md",
        ".hidden/SKILL.md",
        "-flag/SKILL.md",
        "dot./SKILL.md",
        "my skill/SKILL.md",
        "naïve/SKILL.md",
        "con/SKILL.md",
        "LPT1.backup/SKILL.md",
        "Aux/SKILL.md",
        long.as_str(),
    ] {
        assert!(
            matches!(install_name(&skills, path), Err(CodexError::Invalid(_))),
            "accepted {path}"
        );
    }
    for name in ["console", "com", "COM10", "lpt", "nullable"] {
        assert!(validate_name(name).is_ok(), "rejected {name}");
    }
}

#[test]
fn portable_file_names_reject_separators_device_names_and_trailing_dots() {
    for segment in [".gitignore", "run.sh", "naïve.md", "a b.md", "COM10"] {
        assert!(portable_segment(segment), "rejected {segment}");
    }
    for segment in [
        "", ".", "..", "a\\b", "a:b", "a*b", "a?b", "a\"b", "a<b", "a>b", "a|b", "a\nb", "end.",
        "end ", "nul", "CON.txt", "com1.md",
    ] {
        assert!(!portable_segment(segment), "accepted {segment:?}");
    }
}

#[test]
fn lists_only_valid_records_that_name_their_own_folder() {
    let directory = TestDirectory::new();
    let root = &directory.0;
    installed(
        root,
        "test-plan",
        "example/review-tools",
        "skills/test-plan/SKILL.md",
        200,
    );
    installed(
        root,
        "Code-Review",
        "Example/Tools",
        "Code-Review/SKILL.md",
        100,
    );
    installed(
        root,
        "review-tools",
        "example/review-tools",
        "SKILL.md",
        300,
    );
    // Ignored: hidden, renamed, unmarked, corrupt, incompatible, or oversized
    // records, and regular files.
    installed(root, ".hidden", "example/a", "x/.hidden/SKILL.md", 1);
    installed(root, "renamed", "example/a", "skills/original/SKILL.md", 1);
    fs::create_dir(root.join("mine")).unwrap();
    fs::write(root.join("mine/SKILL.md"), "mine").unwrap();
    fs::create_dir(root.join("corrupt")).unwrap();
    fs::write(root.join("corrupt").join(MARKER), "{").unwrap();
    installed(root, "future", "example/a", "future/SKILL.md", 1);
    let marker = root.join("future").join(MARKER);
    let record = fs::read_to_string(&marker)
        .unwrap()
        .replace("\"format_version\":1", "\"format_version\":2");
    fs::write(&marker, record).unwrap();
    installed(
        root,
        "late",
        "example/a",
        "late/SKILL.md",
        MAX_JAVASCRIPT_TIME + 1,
    );
    installed(root, "large", "example/a", "large/SKILL.md", 1);
    fs::write(
        root.join("large").join(MARKER),
        vec![b' '; MAX_MARKER_BYTES as usize + 1],
    )
    .unwrap();
    fs::write(root.join("file"), "file").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(root.join("test-plan"), root.join("linked")).unwrap();

    let codex = CodexSkills::new(root.clone());
    let skills = codex.list().unwrap();
    assert_eq!(
        skills
            .iter()
            .map(|skill| (skill.name.as_str(), skill.repository.as_str()))
            .collect::<Vec<_>>(),
        [
            ("Code-Review", "Example/Tools"),
            ("review-tools", "example/review-tools"),
            ("test-plan", "example/review-tools"),
        ]
    );
    assert_eq!(
        skills[2].link,
        format!("https://github.com/example/review-tools/blob/{COMMIT}/skills/test-plan/SKILL.md")
    );
    assert_eq!(skills[2].installed_at, 200);

    // Repository input forms and case variants address the same installation.
    assert!(replaceable(
        root,
        "test-plan",
        &repository("https://github.com/Example/Review-Tools"),
        "skills/test-plan/SKILL.md"
    )
    .unwrap());
    assert!(matches!(
        replaceable(
            root,
            "test-plan",
            &repository("example/review-tools"),
            "Skills/test-plan/SKILL.md"
        ),
        Err(CodexError::Conflict(_))
    ));
    assert!(!replaceable(root, "absent", &repository("example/a"), "absent/SKILL.md").unwrap());

    // Unlisted folders are never removed.
    for name in ["mine", "corrupt", "renamed", "future"] {
        assert!(matches!(codex.remove(name), Err(CodexError::Conflict(_))));
        assert!(root.join(name).exists());
    }
    #[cfg(unix)]
    {
        assert!(matches!(
            codex.remove("linked"),
            Err(CodexError::Conflict(_))
        ));
        assert!(root.join("test-plan/SKILL.md").exists());
    }
    for name in ["", ".", "..", "../file", "a/b", ".hidden", "nul"] {
        assert!(matches!(codex.remove(name), Err(CodexError::Invalid(_))));
    }
    assert!(root.join(".hidden").exists());
    assert_eq!(
        codex.remove("test-plan").unwrap().unwrap().path,
        "skills/test-plan/SKILL.md"
    );
    assert!(!root.join("test-plan").exists());
    assert!(codex.remove("test-plan").unwrap().is_none());
    assert_eq!(codex.list().unwrap().len(), 2);
}

#[test]
fn disabled_and_missing_directories_are_distinct() {
    let disabled = CodexSkills::disabled();
    assert!(!disabled.is_enabled());
    assert!(disabled.directory().is_none());
    assert!(disabled.list().unwrap().is_empty());
    assert!(matches!(
        disabled.remove("code-review"),
        Err(CodexError::Disabled)
    ));
    let directory = TestDirectory::new();
    let missing = CodexSkills::new(directory.0.join("missing"));
    assert!(missing.list().unwrap().is_empty());
    assert!(missing.remove("code-review").unwrap().is_none());
    assert!(!directory.0.join("missing").exists());
}

#[test]
fn codex_directory_is_the_personal_skills_folder_unless_overridden() {
    let home = if cfg!(target_os = "windows") {
        "USERPROFILE"
    } else {
        "HOME"
    };
    assert_eq!(
        codex_root(|name| (name == home).then(|| OsString::from("/home/user"))),
        Some(PathBuf::from("/home/user").join(".agents").join("skills"))
    );
    assert_eq!(codex_root(|_| None), None);
    assert_eq!(codex_root(|_| Some(OsString::new())), None);
    assert_eq!(
        codex_root(|name| match name {
            "SKILL_SCANNER_CODEX_SKILLS_DIR" => Some(OsString::from("custom")),
            _ => Some(OsString::from("/home/user")),
        }),
        Some(PathBuf::from("custom"))
    );
    assert_eq!(
        codex_root(|name| match name {
            "SKILL_SCANNER_CODEX_SKILLS_DIR" => Some(OsString::new()),
            _ => Some(OsString::from("/home/user")),
        }),
        None
    );
}
