use std::{
    collections::VecDeque,
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::Url;
use serde_json::{json, Value};

use crate::{
    github::{GitHubClient, MAX_SKILL_BYTES},
    report, scanner,
};

#[path = "web_tests.rs"]
mod web;

#[path = "recent_scan_tests.rs"]
mod recent;

#[path = "cache_tests.rs"]
mod cache;

#[path = "organization_tests.rs"]
mod organization_scan;

pub(crate) struct TestDirectory(pub(crate) std::path::PathBuf);

impl TestDirectory {
    pub(crate) fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "skill-scanner-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("could not create test directory: {error}"),
            }
        }
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const COMMIT: &str = "1111111111111111111111111111111111111111";
const ROOT: &str = "2222222222222222222222222222222222222222";
const A: &str = "3333333333333333333333333333333333333333";
const B: &str = "4444444444444444444444444444444444444444";
const C: &str = "5555555555555555555555555555555555555555";
const D: &str = "6666666666666666666666666666666666666666";
const VALID: &[u8] = include_bytes!("../tests/fixtures/valid.md");

struct Reply {
    path: String,
    status: u16,
    body: Vec<u8>,
    headers: Vec<(String, String)>,
    declared_length: Option<usize>,
    delay: Duration,
    body_delay: Duration,
}

impl Reply {
    fn json(path: impl Into<String>, status: u16, value: Value) -> Self {
        Self {
            path: path.into(),
            status,
            body: serde_json::to_vec(&value).unwrap(),
            headers: vec![],
            declared_length: None,
            delay: Duration::ZERO,
            body_delay: Duration::ZERO,
        }
    }

    fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }
}

struct Server {
    url: Url,
    stopped: Arc<AtomicBool>,
    remaining: Arc<Mutex<VecDeque<Reply>>>,
    requests: Arc<Mutex<Vec<String>>>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    fn start(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let stopped = Arc::new(AtomicBool::new(false));
        let remaining = Arc::new(Mutex::new(VecDeque::from(replies)));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = stopped.clone();
        let queue = remaining.clone();
        let captured = requests.clone();
        let thread = thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        // Windows and macOS can inherit the listener's nonblocking
                        // mode; the request reader and response writer are blocking.
                        stream.set_nonblocking(false).unwrap();
                        // Keep separately written headers and bodies independent of
                        // Nagle's algorithm and platform-specific delayed ACK timers.
                        stream.set_nodelay(true).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let mut reader = BufReader::new(stream.try_clone().unwrap());
                        let mut request = String::new();
                        loop {
                            let mut line = String::new();
                            let count = reader.read_line(&mut line).unwrap();
                            if count == 0 || line == "\r\n" {
                                break;
                            }
                            request.push_str(&line);
                        }
                        let reply = queue
                            .lock()
                            .unwrap()
                            .pop_front()
                            .expect("unexpected extra HTTP request");
                        assert_eq!(
                            request.lines().next().unwrap(),
                            format!("GET {} HTTP/1.1", reply.path)
                        );
                        captured.lock().unwrap().push(request);
                        thread::sleep(reply.delay);
                        let mut response = format!("HTTP/1.1 {} Mock\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n", reply.status, reply.declared_length.unwrap_or(reply.body.len()));
                        for (name, value) in reply.headers {
                            response.push_str(&format!("{name}: {value}\r\n"));
                        }
                        response.push_str("\r\n");
                        let _ = stream.write_all(response.as_bytes());
                        thread::sleep(reply.body_delay);
                        let _ = stream.write_all(&reply.body);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1))
                    }
                    Err(error) => panic!("mock server failed: {error}"),
                }
            }
        });
        Self {
            url,
            stopped,
            remaining,
            requests,
            thread: Some(thread),
        }
    }

    fn client(&self) -> GitHubClient {
        GitHubClient::for_test(
            self.url.clone(),
            None,
            Duration::from_secs(2),
            32 * 1024 * 1024,
        )
    }

    fn finish(mut self) -> Vec<String> {
        self.stop();
        assert!(
            self.remaining.lock().unwrap().is_empty(),
            "expected HTTP requests were not made"
        );
        self.requests.lock().unwrap().clone()
    }

    fn stop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let result = thread.join();
            if !thread::panicking() {
                result.unwrap();
            }
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}

fn repository() -> Reply {
    Reply::json(
        "/repos/example/skills",
        200,
        json!({ "private": false, "default_branch": "main" }),
    )
}

fn snapshot() -> Vec<Reply> {
    vec![
        repository(),
        Reply::json(
            "/repos/example/skills/commits/main",
            200,
            json!({ "sha": COMMIT, "commit": { "tree": { "sha": ROOT } } }),
        ),
    ]
}

fn tree(sha: &str, recursive: bool, truncated: bool, entries: Vec<Value>) -> Reply {
    Reply::json(
        format!(
            "/repos/example/skills/git/trees/{sha}{}",
            if recursive { "?recursive=1" } else { "" }
        ),
        200,
        json!({ "sha": sha, "truncated": truncated, "tree": entries }),
    )
}

fn entry(path: &str, sha: &str, mode: &str, kind: &str, size: Option<u64>) -> Value {
    json!({ "path": path, "sha": sha, "mode": mode, "type": kind, "size": size })
}

fn skill(path: &str, sha: &str) -> Value {
    entry(path, sha, "100644", "blob", None)
}

fn blob(sha: &str, content: &[u8]) -> Reply {
    Reply::json(
        format!("/repos/example/skills/git/blobs/{sha}"),
        200,
        json!({ "sha": sha, "encoding": "base64", "size": content.len(), "content": format!("{}\n", STANDARD.encode(content)) }),
    )
}

#[test]
fn scans_root_hidden_nested_and_duplicate_skills_in_stable_order() {
    let mut replies = snapshot();
    replies.push(tree(
        ROOT,
        true,
        false,
        vec![
            skill("z/review/SKILL.md", A),
            entry("link/SKILL.md", B, "120000", "blob", Some(10)),
            entry("submodule", C, "160000", "commit", None),
            skill("SKILL.md", B),
            skill("a/review/SKILL.md", A),
            skill(".hidden/SKILL.md", C),
            skill("skills/skill.md", D),
            entry("executable/SKILL.md", D, "100755", "blob", None),
            entry("SKILL.md-dir", ROOT, "040000", "tree", None),
        ],
    ));
    replies.extend([
        blob(C, include_bytes!("../tests/fixtures/missing.md")),
        blob(B, include_bytes!("../tests/fixtures/malformed.md")),
        blob(A, VALID),
        blob(D, VALID),
        blob(A, VALID),
    ]);
    let server = Server::start(replies);
    let mut progress_output = Vec::new();
    let mut requests_at_progress = Vec::new();
    let inventory = scanner::scan_with_progress(
        &server.client(),
        &"example/skills".parse().unwrap(),
        |progress| {
            requests_at_progress.push(server.requests.lock().unwrap().len());
            report::write_progress(&mut progress_output, progress).unwrap();
        },
    )
    .unwrap();
    let requests = server.finish();
    // Each status must be visible before its HTTP request starts, including
    // repository resolution and discovery before any skill paths are known.
    assert_eq!(requests_at_progress, (0..8).collect::<Vec<_>>());
    let progress_output = String::from_utf8(progress_output).unwrap();
    assert!(progress_output.contains("Scanning skill [1/5]: .hidden/SKILL.md\n"));
    assert!(progress_output.ends_with("Scanning skill [5/5]: z/review/SKILL.md\n"));
    assert_eq!(inventory.commit.as_deref(), Some(COMMIT));
    assert_eq!(
        inventory
            .skills
            .iter()
            .map(|skill| skill.path.as_str())
            .collect::<Vec<_>>(),
        [
            ".hidden/SKILL.md",
            "SKILL.md",
            "a/review/SKILL.md",
            "executable/SKILL.md",
            "z/review/SKILL.md"
        ]
    );
    assert_eq!(inventory.skills[0].name, ".hidden");
    assert_eq!(inventory.skills[0].warnings.len(), 1);
    assert_eq!(inventory.skills[1].name, "skills");
    assert_eq!(inventory.skills[1].warnings.len(), 2);
    assert_eq!(inventory.skills[2].name, inventory.skills[4].name);
    for skill in &inventory.skills {
        assert!(skill.link.contains(&format!("/blob/{COMMIT}/")));
    }
    for request in requests {
        let request = request.to_ascii_lowercase();
        assert!(request.contains("user-agent: skill-scanner/"));
        assert!(request.contains("x-github-api-version: 2022-11-28"));
        assert!(!request.contains("authorization:"));
    }
    let mut output = Vec::new();
    let mut warnings = Vec::new();
    report::write_report(&mut output, &inventory).unwrap();
    report::write_warnings(&mut warnings, &inventory).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("Skills found: 5\n"));
    assert!(!output.contains("Scanning"));
    assert!(String::from_utf8(warnings)
        .unwrap()
        .contains("warning: SKILL.md: name:"));
}

#[test]
fn truncated_recursive_tree_walks_all_directories_and_reuses_shared_trees() {
    let mut replies = snapshot();
    replies.extend([
        tree(ROOT, true, true, vec![skill("partial/SKILL.md", D)]),
        tree(
            ROOT,
            false,
            false,
            vec![
                entry("a", A, "040000", "tree", None),
                entry("b", A, "040000", "tree", None),
            ],
        ),
        tree(
            A,
            false,
            false,
            vec![
                skill("SKILL.md", B),
                entry(".nested", C, "040000", "tree", None),
            ],
        ),
        tree(C, false, false, vec![skill("SKILL.md", D)]),
        blob(D, VALID),
        blob(B, VALID),
        blob(D, VALID),
        blob(B, VALID),
    ]);
    let server = Server::start(replies);
    let mut directories = Vec::new();
    let inventory = scanner::scan_with_progress(
        &server.client(),
        &"example/skills".parse().unwrap(),
        |progress| {
            if let scanner::ScanProgress::Directory(path) = progress {
                directories.push((path.to_owned(), server.requests.lock().unwrap().len()));
            }
        },
    )
    .unwrap();
    // Report directories before fetching them, even when shared trees are cached.
    assert_eq!(
        directories,
        [
            ("/", 3),
            ("b", 4),
            ("b/.nested", 5),
            ("a", 6),
            ("a/.nested", 6)
        ]
        .map(|(path, requests)| (path.to_owned(), requests))
    );
    assert_eq!(
        inventory
            .skills
            .iter()
            .map(|skill| skill.path.as_str())
            .collect::<Vec<_>>(),
        [
            "a/.nested/SKILL.md",
            "a/SKILL.md",
            "b/.nested/SKILL.md",
            "b/SKILL.md"
        ]
    );
    server.finish();
}

#[test]
fn truncated_nonrecursive_tree_and_failed_blob_never_return_an_inventory() {
    let mut replies = snapshot();
    replies.extend([
        tree(ROOT, true, true, vec![]),
        tree(ROOT, false, true, vec![]),
    ]);
    let server = Server::start(replies);
    assert!(
        scanner::scan(&server.client(), &"example/skills".parse().unwrap())
            .unwrap_err()
            .to_string()
            .contains("incomplete")
    );
    server.finish();

    let mut replies = snapshot();
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
            json!({ "message": "Not Found" }),
        ),
    ]);
    let server = Server::start(replies);
    let error = scanner::scan(&server.client(), &"example/skills".parse().unwrap()).unwrap_err();
    assert!(error.to_string().contains("b/SKILL.md"));
    assert!(error.to_string().contains("HTTP 404"));
    server.finish();
}

#[test]
fn distinguishes_empty_repository_no_candidates_and_inaccessible_repository() {
    let repo = "example/skills".parse().unwrap();
    let server = Server::start(vec![
        repository(),
        Reply::json(
            "/repos/example/skills/commits/main",
            409,
            json!({ "message": "Git Repository is empty." }),
        ),
    ]);
    let inventory = scanner::scan(&server.client(), &repo).unwrap();
    assert!(inventory.commit.is_none());
    assert!(inventory.skills.is_empty());
    server.finish();

    let mut replies = snapshot();
    replies.push(tree(ROOT, true, false, vec![skill("README.md", A)]));
    let server = Server::start(replies);
    let inventory = scanner::scan(&server.client(), &repo).unwrap();
    assert_eq!(inventory.commit.as_deref(), Some(COMMIT));
    assert!(inventory.skills.is_empty());
    server.finish();

    for status in [401, 403, 404, 409] {
        let server = Server::start(vec![Reply::json(
            "/repos/example/skills",
            status,
            json!({ "message": "secret-token-must-not-leak" }),
        )]);
        let error = scanner::scan(&server.client(), &repo)
            .unwrap_err()
            .to_string();
        assert!(error.contains(&format!("HTTP {status}")));
        assert!(!error.contains("secret-token-must-not-leak"));
        server.finish();
    }
}

#[test]
fn unknown_commit_conflicts_are_failures_and_private_repositories_are_rejected() {
    let server = Server::start(vec![
        repository(),
        Reply::json(
            "/repos/example/skills/commits/main",
            409,
            json!({ "message": "A different conflict" }),
        ),
    ]);
    assert!(scanner::scan(&server.client(), &"example/skills".parse().unwrap()).is_err());
    server.finish();

    let server = Server::start(vec![Reply::json(
        "/repos/example/skills",
        200,
        json!({ "private": true, "default_branch": "main" }),
    )]);
    let client = GitHubClient::for_test(
        server.url.clone(),
        Some("fake-test-token"),
        Duration::from_secs(2),
        1024,
    );
    assert!(scanner::scan(&client, &"example/skills".parse().unwrap())
        .unwrap_err()
        .to_string()
        .contains("Private repositories"));
    assert!(server.finish()[0].contains("authorization: Bearer fake-test-token"));
}

#[test]
fn oversized_and_non_utf8_files_remain_visible_with_warnings() {
    let mut replies = snapshot();
    replies.extend([
        tree(
            ROOT,
            true,
            false,
            vec![
                entry(
                    "large/SKILL.md",
                    A,
                    "100644",
                    "blob",
                    Some(MAX_SKILL_BYTES + 1),
                ),
                skill("utf8/SKILL.md", B),
                skill("unknown-size/SKILL.md", C),
            ],
        ),
        Reply::json(
            format!("/repos/example/skills/git/blobs/{C}"),
            200,
            json!({ "sha": C, "size": MAX_SKILL_BYTES + 1, "encoding": "base64", "content": "" }),
        ),
        blob(B, &[0xff]),
    ]);
    let server = Server::start(replies);
    let inventory = scanner::scan(&server.client(), &"example/skills".parse().unwrap()).unwrap();
    assert_eq!(inventory.skills.len(), 3);
    for skill in &inventory.skills {
        assert_eq!(skill.warnings.len(), 2);
    }
    server.finish();
}

#[test]
fn retries_transient_statuses_and_honors_small_retry_after() {
    for status in [408, 429, 500, 502, 503, 504] {
        let server = Server::start(vec![
            Reply::json("/repos/example/skills", status, json!({})).header("Retry-After", "0"),
            repository(),
        ]);
        assert!(
            !server
                .client()
                .repository(&"example/skills".parse().unwrap())
                .unwrap()
                .private
        );
        assert_eq!(server.finish().len(), 2);
    }
}

#[test]
fn stops_after_three_attempts_and_does_not_wait_for_long_rate_limits() {
    let server = Server::start(
        (0..3)
            .map(|_| Reply::json("/repos/example/skills", 503, json!({})))
            .collect(),
    );
    assert!(server
        .client()
        .repository(&"example/skills".parse().unwrap())
        .is_err());
    assert_eq!(server.finish().len(), 3);
    for retry_after in ["60", "Wed, 21 Oct 2030 07:28:00 GMT"] {
        let server =
            Server::start(vec![Reply::json("/repos/example/skills", 429, json!({}))
                .header("Retry-After", retry_after)]);
        let error = server
            .client()
            .repository(&"example/skills".parse().unwrap())
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("retry delay"));
        assert_eq!(server.finish().len(), 1);
    }
}

#[test]
fn retries_interrupted_responses_but_rejects_malformed_json_and_oversized_responses() {
    let mut interrupted = repository();
    interrupted.declared_length = Some(interrupted.body.len() + 100);
    let server = Server::start(vec![interrupted, repository()]);
    server
        .client()
        .repository(&"example/skills".parse().unwrap())
        .unwrap();
    assert_eq!(server.finish().len(), 2);

    let mut malformed = repository();
    malformed.body = b"{invalid json".to_vec();
    let server = Server::start(vec![malformed]);
    assert!(server
        .client()
        .repository(&"example/skills".parse().unwrap())
        .is_err());
    server.finish();

    let server = Server::start(vec![repository()]);
    let client = GitHubClient::for_test(server.url.clone(), None, Duration::from_secs(2), 10);
    assert!(client
        .repository(&"example/skills".parse().unwrap())
        .err()
        .unwrap()
        .to_string()
        .contains("limit"));
    server.finish();
}

#[test]
fn follows_only_same_origin_redirects() {
    let server = Server::start(vec![
        Reply::json("/repos/example/skills", 301, json!({}))
            .header("Location", "/repos/new/skills"),
        Reply::json(
            "/repos/new/skills",
            200,
            json!({ "private": false, "default_branch": "main" }),
        ),
    ]);
    server
        .client()
        .repository(&"example/skills".parse().unwrap())
        .unwrap();
    server.finish();
    let server = Server::start(vec![Reply::json("/repos/example/skills", 302, json!({}))
        .header("Location", "https://example.invalid/steal-token")]);
    let client = GitHubClient::for_test(
        server.url.clone(),
        Some("fake-secret"),
        Duration::from_secs(2),
        1024,
    );
    let error = client
        .repository(&"example/skills".parse().unwrap())
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("redirected"));
    assert!(!error.contains("fake-secret"));
    server.finish();
}

#[test]
fn encodes_default_branch_and_rejects_corrupt_listing_and_blob_data() {
    let server = Server::start(vec![Reply::json(
        "/repos/example/skills/commits/feature%2Fskills",
        200,
        json!({ "sha": COMMIT, "commit": { "tree": { "sha": ROOT } } }),
    )]);
    server
        .client()
        .default_commit(&"example/skills".parse().unwrap(), "feature/skills")
        .unwrap();
    server.finish();

    for entries in [
        vec![skill("../SKILL.md", A)],
        vec![skill("SKILL.md", A), skill("SKILL.md", B)],
        vec![entry("SKILL.md", A, "unknown", "blob", None)],
    ] {
        let mut replies = snapshot();
        replies.push(tree(ROOT, true, false, entries));
        let server = Server::start(replies);
        assert!(scanner::scan(&server.client(), &"example/skills".parse().unwrap()).is_err());
        server.finish();
    }
    for value in [
        json!({ "sha": A, "size": 10, "encoding": "base64", "content": "invalid%" }),
        json!({ "sha": A, "size": 10, "encoding": "base64", "content": "eA==" }),
        json!({ "sha": B, "size": 1, "encoding": "base64", "content": "eA==" }),
    ] {
        let server = Server::start(vec![Reply::json(
            format!("/repos/example/skills/git/blobs/{A}"),
            200,
            value,
        )]);
        assert!(server
            .client()
            .blob(&"example/skills".parse().unwrap(), A)
            .is_err());
        server.finish();
    }
}

#[test]
fn request_timeout_covers_headers_and_body_together() {
    let mut slow = repository();
    // Each stage fits within the timeout, but their sum does not. Leave enough
    // scheduling margin for CI runners and for the retry to finish successfully.
    slow.delay = Duration::from_millis(600);
    slow.body_delay = Duration::from_millis(600);
    let server = Server::start(vec![slow, repository()]);
    let client = GitHubClient::for_test(server.url.clone(), None, Duration::from_secs(1), 1024);
    client
        .repository(&"example/skills".parse().unwrap())
        .unwrap();
    assert_eq!(server.finish().len(), 2);
}

#[test]
fn accepts_files_at_exactly_the_metadata_size_limit() {
    let mut bytes = VALID.to_vec();
    bytes.resize(MAX_SKILL_BYTES as usize, b' ');
    let mut replies = snapshot();
    replies.extend([
        tree(
            ROOT,
            true,
            false,
            vec![entry(
                "SKILL.md",
                A,
                "100644",
                "blob",
                Some(MAX_SKILL_BYTES),
            )],
        ),
        blob(A, &bytes),
    ]);
    let server = Server::start(replies);
    let inventory = scanner::scan(&server.client(), &"example/skills".parse().unwrap()).unwrap();
    assert_eq!(inventory.skills[0].name, "code-review");
    assert!(inventory.skills[0].warnings.is_empty());
    server.finish();
}
