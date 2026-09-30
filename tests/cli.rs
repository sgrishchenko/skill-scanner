use std::process::Command;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_skill-scanner"))
        .args(args)
        .env_remove("GITHUB_TOKEN")
        .output()
        .unwrap()
}

#[test]
fn help_and_version_succeed_without_network_or_credentials() {
    for args in [&["--help"][..], &["scan", "--help"][..], &["--version"][..]] {
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
    ] {
        let output = run(args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
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
