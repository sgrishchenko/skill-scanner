use std::process::Command;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_skill-scanner"))
        .args(args)
        .env_remove("GITHUB_TOKEN")
        .env("SKILL_SCANNER_HISTORY_DIR", "")
        .env("SKILL_SCANNER_CACHE_DIR", "")
        .env("SKILL_SCANNER_STARRED_DIR", "")
        .env("SKILL_SCANNER_CODEX_SKILLS_DIR", "")
        .output()
        .unwrap()
}

#[test]
fn help_and_version_succeed_without_network_or_credentials() {
    for args in [
        &["--help"][..],
        &["scan", "--help"][..],
        &["scan-org", "--help"][..],
        &["serve", "--help"][..],
        &["recent", "--help"][..],
        &["recent", "remove", "--help"][..],
        &["starred", "--help"][..],
        &["starred", "remove", "--help"][..],
        &["codex", "--help"][..],
        &["codex", "install", "--help"][..],
        &["codex", "remove", "--help"][..],
        &["--version"][..],
    ] {
        let output = run(args);
        assert!(output.status.success());
        assert!(!output.stdout.is_empty());
        assert!(output.stderr.is_empty());
        assert!(!output.stdout.contains(&0x1b));
    }
}

#[test]
fn invalid_usage_exits_two_and_never_prints_an_inventory() {
    for args in [
        &[][..],
        &["scan"][..],
        &["scan", "../local"][..],
        &["scan", "a/b/tree/main"][..],
        &["scan", "a/b", "c/d"][..],
        &["scan", "a/b", "--json"][..],
        &["serve", "--port", "65536"][..],
        &["serve", "--port", "invalid"][..],
        &["serve", "--host", "0.0.0.0"][..],
        &["recent", "remove"][..],
        &["recent", "remove", "../escape"][..],
        &["scan-org"][..],
        &["scan-org", "example/skills"][..],
        &["scan-org", "https://github.com/orgs/example"][..],
        &["scan-org", "example", "other"][..],
        &["starred", "remove"][..],
        &["starred", "remove", "a/b"][..],
        &["starred", "remove", "../escape", "SKILL.md"][..],
        &["starred", "remove", "a/b", "../SKILL.md"][..],
        &["starred", "remove", "a/b", "README.md"][..],
        &["codex", "install"][..],
        &["codex", "install", "a/b"][..],
        &["codex", "install", "../escape", "SKILL.md"][..],
        &["codex", "install", "a/b", "../SKILL.md"][..],
        &["codex", "install", "a/b", "README.md"][..],
        &["codex", "install", "a/b", "my skill/SKILL.md"][..],
        &["codex", "remove"][..],
        &["codex", "remove", "../escape"][..],
        &["codex", "remove", "a", "b"][..],
        &["codex", "list"][..],
    ] {
        let output = run(args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn recent_commands_persist_removal_and_work_without_valid_github_credentials() {
    use skill_scanner::{recent::RecentRepositories, scanner::Inventory};
    let directory =
        std::env::temp_dir().join(format!("skill-scanner-cli-recent-{}", std::process::id()));
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    std::fs::create_dir(&directory).unwrap();
    let directory = Directory(directory);
    let recent = RecentRepositories::new(directory.0.clone());
    recent
        .record(&Inventory {
            repository: "example/skills".parse().unwrap(),
            commit: None,
            skills: vec![],
        })
        .unwrap();
    let invoke = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_skill-scanner"))
            .args(args)
            .env("SKILL_SCANNER_HISTORY_DIR", &directory.0)
            .env("GITHUB_TOKEN", "secret\ninvalid-header")
            .output()
            .unwrap()
    };
    let output = invoke(&["recent"]);
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("example/skills  (0 skills)"));
    assert!(output.stderr.is_empty());
    let output = invoke(&["recent", "remove", "https://github.com/EXAMPLE/Skills.git/"]);
    assert!(output.status.success());
    assert!(recent.list().unwrap().is_empty());
    let output = invoke(&["recent"]);
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("No recently scanned repositories."));
    let output = invoke(&["recent", "remove", "https://user:secret@github.com/a/b"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(!String::from_utf8(output.stderr).unwrap().contains("secret"));
    let output = Command::new(env!("CARGO_BIN_EXE_skill-scanner"))
        .arg("recent")
        .env("SKILL_SCANNER_HISTORY_DIR", "")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("disabled"));
    let file = directory.0.join("file");
    std::fs::write(&file, "file").unwrap();
    for args in [&["recent"][..], &["recent", "remove", "example/skills"][..]] {
        let output = Command::new(env!("CARGO_BIN_EXE_skill-scanner"))
            .args(args)
            .env("SKILL_SCANNER_HISTORY_DIR", &file)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr)
            .unwrap()
            .contains("SKILL_SCANNER_HISTORY_DIR"));
    }
    assert_eq!(std::fs::read_to_string(file).unwrap(), "file");
}

#[test]
fn starred_commands_list_and_persist_removal_without_github_access() {
    use skill_scanner::starred::StarredSkills;
    let directory =
        std::env::temp_dir().join(format!("skill-scanner-cli-starred-{}", std::process::id()));
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    std::fs::create_dir(&directory).unwrap();
    let directory = Directory(directory);
    let starred = StarredSkills::new(directory.0.clone());
    let commit = "1111111111111111111111111111111111111111";
    starred
        .star(
            &"example/skills".parse().unwrap(),
            "skills/review/SKILL.md",
            "review\u{1b}[31m",
            commit,
        )
        .unwrap();
    let invoke = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_skill-scanner"))
            .args(args)
            .env("SKILL_SCANNER_STARRED_DIR", &directory.0)
            .env("GITHUB_TOKEN", "secret\ninvalid-header")
            .env("HTTPS_PROXY", "http://127.0.0.1:9")
            .output()
            .unwrap()
    };
    let output = invoke(&["starred"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(!output.stdout.contains(&0x1b));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!(
            "Starred skills (newest first):\n\nreview\\u{{1b}}[31m\n  Repository: example/skills\n  Path: skills/review/SKILL.md\n  Link: https://github.com/example/skills/blob/{commit}/skills/review/SKILL.md\n"
        )
    );
    for _ in 0..2 {
        let output = invoke(&[
            "starred",
            "remove",
            "https://github.com/EXAMPLE/Skills.git/",
            "skills/review/SKILL.md",
        ]);
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "Unstarred skills/review/SKILL.md in EXAMPLE/Skills.\n"
        );
    }
    assert!(starred.list().unwrap().is_empty());
    let output = invoke(&["starred"]);
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("No starred skills."));
    let output = run(&["starred"]);
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("disabled"));
    let file = directory.0.join("file");
    std::fs::write(&file, "file").unwrap();
    for args in [
        &["starred"][..],
        &["starred", "remove", "example/skills", "SKILL.md"][..],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_skill-scanner"))
            .args(args)
            .env("SKILL_SCANNER_STARRED_DIR", &file)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr)
            .unwrap()
            .contains("SKILL_SCANNER_STARRED_DIR"));
    }
    assert_eq!(std::fs::read_to_string(file).unwrap(), "file");
}

#[test]
fn codex_commands_list_and_remove_only_skill_scanner_installations_without_github_access() {
    let directory =
        std::env::temp_dir().join(format!("skill-scanner-cli-codex-{}", std::process::id()));
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    std::fs::create_dir(&directory).unwrap();
    let directory = Directory(directory);
    let commit = "1111111111111111111111111111111111111111";
    let installed = directory.0.join("code-review");
    std::fs::create_dir(&installed).unwrap();
    std::fs::write(installed.join("SKILL.md"), "---\nname: code-review\n---\n").unwrap();
    std::fs::write(
        installed.join(".skill-scanner.json"),
        format!(
            r#"{{"format_version":1,"repository":"example/skills","path":"skills/code-review/SKILL.md","commit":"{commit}","installed_at":1}}"#
        ),
    )
    .unwrap();
    std::fs::create_dir(directory.0.join("mine")).unwrap();
    let invoke = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_skill-scanner"))
            .args(args)
            .env("SKILL_SCANNER_CODEX_SKILLS_DIR", &directory.0)
            .env("GITHUB_TOKEN", "secret\ninvalid-header")
            .env("HTTPS_PROXY", "http://127.0.0.1:9")
            .output()
            .unwrap()
    };
    let output = invoke(&["codex"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!(
            "Codex skills installed by Skill Scanner in {}:\n\ncode-review\n  Repository: example/skills\n  Path: skills/code-review/SKILL.md\n  Link: https://github.com/example/skills/blob/{commit}/skills/code-review/SKILL.md\n",
            directory.0.display()
        )
    );
    let output = invoke(&["codex", "remove", "mine"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("was not installed by Skill Scanner"));
    assert!(directory.0.join("mine").exists());
    let output = invoke(&["codex", "remove", "code-review"]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Removed code-review from Codex skills.\n"
    );
    assert!(!installed.exists());
    let output = invoke(&["codex", "remove", "code-review"]);
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("nothing was removed"));
    let output = invoke(&["codex"]);
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .starts_with("No Codex skills installed by Skill Scanner"));

    // Disabled installation never contacts GitHub.
    for args in [
        &["codex"][..],
        &["codex", "install", "example/skills", "SKILL.md"][..],
    ] {
        let output = run(args);
        let text = String::from_utf8([output.stdout, output.stderr].concat()).unwrap();
        assert!(text.contains("disabled"), "{text}");
    }
    assert_eq!(
        run(&["codex", "install", "example/skills", "SKILL.md"])
            .status
            .code(),
        Some(1)
    );
    let file = directory.0.join("file");
    std::fs::write(&file, "file").unwrap();
    for args in [&["codex"][..], &["codex", "remove", "code-review"][..]] {
        let output = Command::new(env!("CARGO_BIN_EXE_skill-scanner"))
            .args(args)
            .env("SKILL_SCANNER_CODEX_SKILLS_DIR", &file)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr)
            .unwrap()
            .contains("SKILL_SCANNER_CODEX_SKILLS_DIR"));
    }
    assert_eq!(std::fs::read_to_string(file).unwrap(), "file");
    let output = run(&[
        "codex",
        "install",
        "https://user:secret@github.com/a/b",
        "SKILL.md",
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(!String::from_utf8(output.stderr).unwrap().contains("secret"));
}

#[test]
fn invalid_credentials_exit_one_without_echoing_the_secret() {
    let output = Command::new(env!("CARGO_BIN_EXE_skill-scanner"))
        .args(["scan", "example/skills"])
        .env("GITHUB_TOKEN", "secret\ninvalid-header")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("GITHUB_TOKEN"));
    assert!(!stderr.contains("secret"));
}

#[test]
fn rejects_credentials_in_urls_without_echoing_them() {
    let output = run(&["scan", "https://username:secret@github.com/example/skills"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stderr.contains("username"));
    assert!(!stderr.contains("secret"));
}

#[test]
fn serve_starts_on_an_available_port_and_serves_assets_from_any_directory() {
    use std::{
        io::{BufRead, BufReader},
        process::Stdio,
        time::Duration,
    };

    struct Server(std::process::Child);
    impl Drop for Server {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut server = Server(
        Command::new(env!("CARGO_BIN_EXE_skill-scanner"))
            .args(["serve", "--port", "0"])
            .env_remove("GITHUB_TOKEN")
            .env("SKILL_SCANNER_HISTORY_DIR", "")
            .env("SKILL_SCANNER_CACHE_DIR", "")
            .current_dir(std::env::temp_dir())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut output = BufReader::new(server.0.stderr.take().unwrap());
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    let url = line
        .trim()
        .strip_prefix("Skill Scanner is available at ")
        .unwrap();
    assert!(url.starts_with("http://127.0.0.1:"));
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let response = client.get(url).send().unwrap();
    assert!(response.status().is_success());
    assert!(response.text().unwrap().contains("GitHub repository"));
    let response = client.get(format!("{url}/app.js")).send().unwrap();
    assert!(response.status().is_success());
    assert!(response.text().unwrap().contains("/api/scan"));
}

#[test]
fn occupied_web_port_fails_with_an_actionable_error() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let output = run(&[
        "serve",
        "--port",
        &listener.local_addr().unwrap().port().to_string(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("could not run the web interface"));
}

#[test]
fn rejected_organization_inputs_are_not_echoed() {
    let output = run(&["scan-org", "https://name:secret@github.com/example"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("expected OWNER or https://github.com/OWNER"));
    assert!(!stderr.contains("secret"));
}
