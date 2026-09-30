use std::process::Command;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_skill-scanner"))
        .args(args)
        .env_remove("GITHUB_TOKEN")
        .env("SKILL_SCANNER_HISTORY_DIR", "")
        .env("SKILL_SCANNER_CACHE_DIR", "")
        .output()
        .unwrap()
}

#[test]
fn help_and_version_succeed_without_network_or_credentials() {
    for args in [
        &["--help"][..],
        &["scan", "--help"][..],
        &["serve", "--help"][..],
        &["recent", "--help"][..],
        &["recent", "remove", "--help"][..],
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
