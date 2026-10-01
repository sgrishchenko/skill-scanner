use super::*;
use crate::{
    codex::{CodexError, CodexSkills, InstallProgress, MARKER, MAX_FILES, MAX_INSTALL_BYTES},
    test_support::TestDirectory,
};
use std::{fs, path::Path};

const SCRIPT: &[u8] = b"#!/bin/sh\necho repository script\n";

fn pinned_commit() -> Reply {
    Reply::json(
        format!("/repos/example/skills/commits/{COMMIT}"),
        200,
        json!({ "sha": COMMIT, "commit": { "tree": { "sha": ROOT } } }),
    )
}

fn directory(name: &str, sha: &str) -> Value {
    entry(name, sha, "040000", "tree", None)
}

/// Repository details, the pinned commit, and the walk to skills/code-review.
fn install_listing(skill_entries: Vec<Value>) -> Vec<Reply> {
    vec![
        repository(),
        pinned_commit(),
        tree(ROOT, false, false, vec![directory("skills", A)]),
        tree(
            A,
            false,
            false,
            vec![directory("code-review", B), directory("other", C)],
        ),
        tree(B, true, false, skill_entries),
    ]
}

fn sized(path: &str, sha: &str, mode: &str, content: &[u8]) -> Value {
    entry(path, sha, mode, "blob", Some(content.len() as u64))
}

fn install(
    codex: &CodexSkills,
    server: &Server,
    path: &str,
    commit: Option<&str>,
) -> Result<crate::codex::Installation, CodexError> {
    codex.install(
        &server.client(),
        &"example/skills".parse().unwrap(),
        path,
        commit,
        |_| {},
    )
}

fn entries(path: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(path)
        .map(|entries| {
            entries
                .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn installs_the_skill_directory_without_nested_skills_symlinks_submodules_or_records() {
    let root = TestDirectory::new();
    let codex = CodexSkills::new(root.0.join("skills"));
    let mut replies = install_listing(vec![
        sized("SKILL.md", C, "100644", VALID),
        directory("scripts", D),
        sized("scripts/run.sh", D, "100755", SCRIPT),
        sized("references/guide.md", A, "100644", b"guide"),
        entry("link", A, "120000", "blob", Some(4)),
        entry("vendor", A, "160000", "commit", None),
        sized("nested/SKILL.md", C, "100644", VALID),
        sized("nested/notes.md", A, "100644", b"nested"),
        sized(MARKER, A, "100644", b"{}"),
    ]);
    replies.extend([blob(C, VALID), blob(D, SCRIPT), blob(A, b"guide")]);
    let server = Server::start(replies);
    let mut progress = Vec::new();
    let installation = codex
        .install(
            &server.client(),
            &"https://github.com/example/skills.git".parse().unwrap(),
            "skills/code-review/SKILL.md",
            Some(COMMIT),
            |event| {
                report::write_install_progress(&mut progress, event).unwrap();
            },
        )
        .unwrap();
    let requests = server.finish();
    assert_eq!(requests.len(), 8);
    assert_eq!(
        String::from_utf8(progress).unwrap(),
        format!(
            "Resolving repository: example/skills\nResolving commit: {COMMIT}\nListing skill files: skills/code-review\nDownloading file [1/3]: skills/code-review/SKILL.md\nDownloading file [2/3]: skills/code-review/scripts/run.sh\nDownloading file [3/3]: skills/code-review/references/guide.md\n"
        )
    );
    let installed = root.0.join("skills/code-review");
    assert_eq!(installation.directory, installed);
    assert_eq!(installation.files, 3);
    assert_eq!(installation.skill.name, "code-review");
    assert_eq!(installation.skill.repository, "example/skills");
    assert_eq!(installation.skill.commit, COMMIT);
    assert_eq!(
        installation.skill.link,
        format!("https://github.com/example/skills/blob/{COMMIT}/skills/code-review/SKILL.md")
    );
    assert_eq!(
        entries(&installed),
        [MARKER, "SKILL.md", "references", "scripts"]
    );
    assert_eq!(fs::read(installed.join("SKILL.md")).unwrap(), VALID);
    assert_eq!(fs::read(installed.join("scripts/run.sh")).unwrap(), SCRIPT);
    assert_eq!(
        fs::read(installed.join("references/guide.md")).unwrap(),
        b"guide"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |path: &str| {
            fs::metadata(installed.join(path))
                .unwrap()
                .permissions()
                .mode()
        };
        assert_ne!(mode("scripts/run.sh") & 0o111, 0);
        assert_eq!(mode("SKILL.md") & 0o111, 0);
    }
    // Only the installed folder remains; staging folders are gone.
    assert_eq!(entries(&root.0.join("skills")), ["code-review"]);
    assert_eq!(codex.list().unwrap(), [installation.skill]);
}

#[test]
fn reinstalling_the_same_skill_replaces_its_folder_with_the_default_branch() {
    let root = TestDirectory::new();
    let codex = CodexSkills::new(root.0.clone());
    let mut replies = install_listing(vec![
        sized("SKILL.md", C, "100644", VALID),
        sized("old.md", A, "100644", b"old"),
    ]);
    replies.extend([blob(C, VALID), blob(A, b"old")]);
    let server = Server::start(replies);
    install(&codex, &server, "skills/code-review/SKILL.md", Some(COMMIT)).unwrap();
    server.finish();

    // The CLI installs the current default-branch commit.
    let newer = "7".repeat(40);
    let server = Server::start(vec![
        repository(),
        Reply::json(
            "/repos/example/skills/commits/main",
            200,
            json!({ "sha": newer, "commit": { "tree": { "sha": ROOT } } }),
        ),
        tree(ROOT, false, false, vec![directory("skills", A)]),
        tree(A, false, false, vec![directory("code-review", B)]),
        tree(
            B,
            true,
            false,
            vec![
                sized("SKILL.md", C, "100644", VALID),
                sized("new.md", D, "100644", b"new"),
            ],
        ),
        blob(C, VALID),
        blob(D, b"new"),
    ]);
    let installation = codex
        .install(
            &server.client(),
            &"example/skills".parse().unwrap(),
            "skills/code-review/SKILL.md",
            None,
            |_| {},
        )
        .unwrap();
    server.finish();
    assert_eq!(installation.skill.commit, newer);
    let installed = root.0.join("code-review");
    assert_eq!(entries(&installed), [MARKER, "SKILL.md", "new.md"]);
    assert_eq!(entries(&root.0), ["code-review"]);
    assert_eq!(codex.list().unwrap()[0].commit, newer);
}

#[test]
fn root_level_skills_install_under_the_repository_name() {
    let root = TestDirectory::new();
    let codex = CodexSkills::new(root.0.clone());
    let server = Server::start(vec![
        repository(),
        pinned_commit(),
        tree(
            ROOT,
            true,
            false,
            vec![
                sized("SKILL.md", C, "100644", VALID),
                directory("skills", A),
                sized("skills/review/SKILL.md", C, "100644", VALID),
            ],
        ),
        blob(C, VALID),
    ]);
    let mut listed = Vec::new();
    let installation = codex
        .install(
            &server.client(),
            &"example/skills".parse().unwrap(),
            "SKILL.md",
            Some(COMMIT),
            |event| {
                if let InstallProgress::Listing(directory) = event {
                    listed.push(directory.to_owned());
                }
            },
        )
        .unwrap();
    server.finish();
    assert_eq!(listed, ["/"]);
    assert_eq!(installation.skill.name, "skills");
    assert_eq!(entries(&root.0.join("skills")), [MARKER, "SKILL.md"]);
}

#[test]
fn existing_folders_from_other_sources_conflict_before_any_github_request() {
    let root = TestDirectory::new();
    let codex = CodexSkills::new(root.0.clone());
    fs::create_dir(root.0.join("code-review")).unwrap();
    fs::write(root.0.join("code-review/SKILL.md"), "mine").unwrap();
    let server = Server::start(vec![]);
    let error = install(&codex, &server, "skills/code-review/SKILL.md", Some(COMMIT)).unwrap_err();
    assert!(matches!(error, CodexError::Conflict(_)));
    assert!(error.to_string().contains("not installed by Skill Scanner"));
    let error = codex.remove("code-review").unwrap_err();
    assert!(matches!(error, CodexError::Conflict(_)));
    assert_eq!(
        fs::read_to_string(root.0.join("code-review/SKILL.md")).unwrap(),
        "mine"
    );
    fs::remove_dir_all(root.0.join("code-review")).unwrap();

    // A skill with the same folder name from another repository is kept too.
    let mut replies = install_listing(vec![sized("SKILL.md", C, "100644", VALID)]);
    replies.push(blob(C, VALID));
    let server = Server::start(replies);
    install(&codex, &server, "skills/code-review/SKILL.md", Some(COMMIT)).unwrap();
    server.finish();
    let server = Server::start(vec![]);
    let error = codex
        .install(
            &server.client(),
            &"other/tools".parse().unwrap(),
            "code-review/SKILL.md",
            Some(COMMIT),
            |_| {},
        )
        .unwrap_err();
    server.finish();
    assert!(matches!(error, CodexError::Conflict(_)));
    assert!(error
        .to_string()
        .contains("already holds skills/code-review/SKILL.md from example/skills"));
    assert_eq!(codex.list().unwrap()[0].repository, "example/skills");
    assert_eq!(
        codex.remove("code-review").unwrap().unwrap().path,
        "skills/code-review/SKILL.md"
    );
    assert!(entries(&root.0).is_empty());
    assert!(codex.remove("code-review").unwrap().is_none());
}

#[test]
fn unsafe_oversized_and_changed_skills_are_rejected_without_writing_files() {
    let skill = || sized("SKILL.md", C, "100644", VALID);
    let many: Vec<Value> = std::iter::once(skill())
        .chain((0..MAX_FILES).map(|index| sized(&format!("f{index}.md"), A, "100644", b"x")))
        .collect();
    for (listing, message) in [
        (vec![skill(), sized("a\\b.md", A, "100644", b"x")], "safely"),
        (vec![skill(), sized("c:d.md", A, "100644", b"x")], "safely"),
        (
            vec![skill(), sized("docs/CON.txt", A, "100644", b"x")],
            "safely",
        ),
        (
            vec![skill(), sized("trailing.", A, "100644", b"x")],
            "safely",
        ),
        (
            vec![
                skill(),
                sized("Guide.md", A, "100644", b"x"),
                sized("guide.md", A, "100644", b"x"),
            ],
            "safely",
        ),
        (many, "too large"),
        (
            vec![
                skill(),
                entry("big.bin", A, "100644", "blob", Some(MAX_INSTALL_BYTES)),
            ],
            "too large",
        ),
        (
            vec![entry("SKILL.md", C, "120000", "blob", Some(8))],
            "not a regular file",
        ),
        (
            vec![skill(), sized("../escape.md", A, "100644", b"x")],
            "invalid",
        ),
    ] {
        let root = TestDirectory::new();
        let codex = CodexSkills::new(root.0.join("skills"));
        let server = Server::start(install_listing(listing));
        let error = install(&codex, &server, "skills/code-review/SKILL.md", Some(COMMIT))
            .unwrap_err()
            .to_string();
        server.finish();
        assert!(error.contains(message), "{error}");
        assert!(!root.0.join("skills").exists());
    }

    // A truncated listing, a missing directory, and a different commit fail too.
    let root = TestDirectory::new();
    let codex = CodexSkills::new(root.0.clone());
    let mut replies = install_listing(vec![]);
    replies.pop();
    replies.push(tree(B, true, true, vec![]));
    let server = Server::start(replies);
    assert!(
        install(&codex, &server, "skills/code-review/SKILL.md", Some(COMMIT))
            .unwrap_err()
            .to_string()
            .contains("too large")
    );
    server.finish();
    let mut replies = install_listing(vec![]);
    replies.truncate(4);
    let server = Server::start(replies);
    assert!(
        install(&codex, &server, "skills/missing/SKILL.md", Some(COMMIT))
            .unwrap_err()
            .to_string()
            .contains("does not exist")
    );
    server.finish();
    let server = Server::start(vec![
        repository(),
        Reply::json(
            format!("/repos/example/skills/commits/{COMMIT}"),
            200,
            json!({ "sha": "8".repeat(40), "commit": { "tree": { "sha": ROOT } } }),
        ),
    ]);
    assert!(
        install(&codex, &server, "skills/code-review/SKILL.md", Some(COMMIT))
            .unwrap_err()
            .to_string()
            .contains("different commit")
    );
    server.finish();
    assert!(entries(&root.0).is_empty());
}

#[test]
fn failed_downloads_keep_the_previous_installation_and_leave_no_partial_copy() {
    let root = TestDirectory::new();
    let codex = CodexSkills::new(root.0.clone());
    let mut replies = install_listing(vec![sized("SKILL.md", C, "100644", VALID)]);
    replies.push(blob(C, VALID));
    let server = Server::start(replies);
    install(&codex, &server, "skills/code-review/SKILL.md", Some(COMMIT)).unwrap();
    server.finish();

    let mut replies = install_listing(vec![
        sized("SKILL.md", C, "100644", VALID),
        sized("extra.md", D, "100644", b"extra"),
    ]);
    replies.extend([
        blob(C, VALID),
        Reply::json(
            format!("/repos/example/skills/git/blobs/{D}"),
            404,
            json!({ "message": "secret-detail" }),
        ),
    ]);
    let server = Server::start(replies);
    let error = install(&codex, &server, "skills/code-review/SKILL.md", Some(COMMIT)).unwrap_err();
    server.finish();
    assert!(matches!(error, CodexError::Source(_)));
    let error = error.to_string();
    assert!(error.contains("skills/code-review/extra.md"));
    assert!(error.contains("HTTP 404"));
    assert!(!error.contains("secret-detail"));
    assert_eq!(entries(&root.0), ["code-review"]);
    assert_eq!(entries(&root.0.join("code-review")), [MARKER, "SKILL.md"]);

    // Private and empty repositories never install anything.
    let server = Server::start(vec![Reply::json(
        "/repos/example/skills",
        200,
        json!({ "private": true, "default_branch": "main" }),
    )]);
    assert!(install(&codex, &server, "skills/other/SKILL.md", None)
        .unwrap_err()
        .to_string()
        .contains("Private repositories"));
    server.finish();
    let server = Server::start(vec![
        repository(),
        Reply::json(
            "/repos/example/skills/commits/main",
            409,
            json!({ "message": "Git Repository is empty." }),
        ),
    ]);
    assert!(install(&codex, &server, "skills/other/SKILL.md", None)
        .unwrap_err()
        .to_string()
        .contains("empty"));
    server.finish();
    assert_eq!(entries(&root.0), ["code-review"]);
}

#[test]
fn invalid_requests_and_disabled_installation_make_no_github_requests() {
    let root = TestDirectory::new();
    let codex = CodexSkills::new(root.0.clone());
    let server = Server::start(vec![]);
    for (path, commit) in [
        ("README.md", Some(COMMIT)),
        ("../SKILL.md", Some(COMMIT)),
        (".hidden/SKILL.md", Some(COMMIT)),
        ("my skill/SKILL.md", Some(COMMIT)),
        ("skills/nul/SKILL.md", Some(COMMIT)),
        ("skills/review/SKILL.md", Some("main")),
        ("skills/review/SKILL.md", Some(&COMMIT[1..])),
    ] {
        assert!(matches!(
            install(&codex, &server, path, commit),
            Err(CodexError::Invalid(_))
        ));
    }
    assert!(matches!(
        install(
            &CodexSkills::disabled(),
            &server,
            "skills/review/SKILL.md",
            None
        ),
        Err(CodexError::Disabled)
    ));
    server.finish();
    assert!(entries(&root.0).is_empty());

    // A regular file used as the directory is a storage error and is preserved.
    let file = root.0.join("file");
    fs::write(&file, "file").unwrap();
    let codex = CodexSkills::new(file.clone());
    let server = Server::start(vec![]);
    assert!(matches!(
        install(&codex, &server, "skills/review/SKILL.md", None),
        Err(CodexError::Storage(_))
    ));
    server.finish();
    assert!(codex.list().is_err());
    assert!(matches!(
        codex.remove("review"),
        Err(CodexError::Storage(_))
    ));
    assert_eq!(fs::read_to_string(file).unwrap(), "file");
}
