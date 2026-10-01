use super::*;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    response::Response,
    Router,
};
use tower::ServiceExt;

const HOST: &str = "127.0.0.1:3000";

fn with_web(replies: Vec<Reply>, test: impl FnOnce(Router, &tokio::runtime::Runtime)) {
    with_web_cache(replies, crate::cache::ScanCache::disabled(), test);
}

fn with_web_cache(
    replies: Vec<Reply>,
    cache: crate::cache::ScanCache,
    test: impl FnOnce(Router, &tokio::runtime::Runtime),
) {
    with_web_storage(
        replies,
        cache,
        crate::recent::RecentRepositories::disabled(),
        test,
    );
}

fn with_web_storage(
    replies: Vec<Reply>,
    cache: crate::cache::ScanCache,
    recent: crate::recent::RecentRepositories,
    test: impl FnOnce(Router, &tokio::runtime::Runtime),
) {
    with_web_stores(
        replies,
        cache,
        recent,
        crate::starred::StarredSkills::disabled(),
        test,
    );
}

fn with_web_stores(
    replies: Vec<Reply>,
    cache: crate::cache::ScanCache,
    recent: crate::recent::RecentRepositories,
    starred: crate::starred::StarredSkills,
    test: impl FnOnce(Router, &tokio::runtime::Runtime),
) {
    with_web_codex(
        replies,
        cache,
        recent,
        starred,
        crate::codex::CodexSkills::disabled(),
        test,
    );
}

fn with_web_codex(
    replies: Vec<Reply>,
    cache: crate::cache::ScanCache,
    recent: crate::recent::RecentRepositories,
    starred: crate::starred::StarredSkills,
    codex: crate::codex::CodexSkills,
    test: impl FnOnce(Router, &tokio::runtime::Runtime),
) {
    let server = Server::start(replies);
    // The blocking reqwest client must be created and dropped outside Tokio.
    let client = Arc::new(server.client());
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    test(
        crate::web::router(client.clone(), 3000, cache, recent, starred, codex),
        &runtime,
    );
    drop(runtime);
    server.finish();
}

fn request(method: &str, path: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("host", HOST)
        .header("origin", format!("http://{HOST}"))
        .header("content-type", "application/json")
        .header("x-skill-scanner", "1")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

fn scan_request() -> Request<Body> {
    request("POST", "/api/scan", r#"{"repository":"example/skills"}"#)
}

async fn events(response: Response) -> Vec<Value> {
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["content-type"],
        "application/x-ndjson; charset=utf-8"
    );
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    std::str::from_utf8(&bytes)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn assets_are_embedded_and_local_requests_have_browser_protections() {
    with_web(vec![], |app, runtime| {
        runtime.block_on(async {
            for (path, mime, content) in [
                ("/", "text/html; charset=utf-8", "GitHub repository"),
                ("/app.js", "text/javascript; charset=utf-8", "textContent"),
                (
                    "/app.css",
                    "text/css; charset=utf-8",
                    "prefers-reduced-motion",
                ),
                ("/favicon.svg", "image/svg+xml", "<svg"),
            ] {
                let response = app.clone().oneshot(request("GET", path, "")).await.unwrap();
                assert_eq!(response.status(), StatusCode::OK);
                assert_eq!(response.headers()["content-type"], mime);
                assert_eq!(response.headers()["cache-control"], "no-store");
                assert_eq!(response.headers()["x-content-type-options"], "nosniff");
                assert!(response.headers()["content-security-policy"]
                    .to_str()
                    .unwrap()
                    .contains("frame-ancestors 'none'"));
                assert!(!response
                    .headers()
                    .contains_key("access-control-allow-origin"));
                let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
                assert!(std::str::from_utf8(&bytes).unwrap().contains(content));
            }
            let response = app
                .oneshot(request("GET", "/../Cargo.toml", ""))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        })
    });
}

#[test]
fn rejects_cross_origin_rebinding_and_unmarked_scan_requests() {
    with_web(vec![], |app, runtime| {
        runtime.block_on(async {
            let mut foreign_origin = scan_request();
            foreign_origin
                .headers_mut()
                .insert("origin", "https://other.example".parse().unwrap());
            let mut rebinding = request("GET", "/", "");
            rebinding
                .headers_mut()
                .insert("host", "other.example:3000".parse().unwrap());
            let mut unmarked = scan_request();
            unmarked.headers_mut().remove("x-skill-scanner");
            let mut no_host = scan_request();
            no_host.headers_mut().remove("host");
            for request in [foreign_origin, rebinding, unmarked, no_host] {
                assert_eq!(
                    app.clone().oneshot(request).await.unwrap().status(),
                    StatusCode::FORBIDDEN
                );
            }
            let mut localhost = request("GET", "/", "");
            localhost
                .headers_mut()
                .insert("host", "localhost:3000".parse().unwrap());
            localhost
                .headers_mut()
                .insert("origin", "http://localhost:3000".parse().unwrap());
            assert_eq!(
                app.oneshot(localhost).await.unwrap().status(),
                StatusCode::OK
            );
        })
    });
}

#[test]
fn default_http_port_accepts_the_hosts_and_origins_browsers_send() {
    let server = Server::start(vec![]);
    let client = Arc::new(server.client());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let app = crate::web::router(
            client.clone(),
            80,
            crate::cache::ScanCache::disabled(),
            crate::recent::RecentRepositories::disabled(),
            crate::starred::StarredSkills::disabled(),
            crate::codex::CodexSkills::disabled(),
        );
        for host in ["127.0.0.1", "localhost"] {
            let mut request = request("GET", "/", "");
            request.headers_mut().insert("host", host.parse().unwrap());
            request
                .headers_mut()
                .insert("origin", format!("http://{host}").parse().unwrap());
            assert_eq!(
                app.clone().oneshot(request).await.unwrap().status(),
                StatusCode::OK
            );
        }
    });
    server.finish();
}

#[test]
fn rejects_invalid_inputs_without_contacting_github_or_echoing_secrets() {
    with_web(vec![], |app, runtime| {
        runtime.block_on(async {
            for body in [
                r#"{"repository":"https://name:secret@github.com/a/b"}"#,
                r#"{"repository":"a/b/tree/main"}"#,
                r#"{"repository":false}"#,
                r#"{"repository":"a/b","token":"secret"}"#,
                "{}",
                "{invalid",
            ] {
                let response = app
                    .clone()
                    .oneshot(request("POST", "/api/scan", body))
                    .await
                    .unwrap();
                assert!(response.status().is_client_error());
                let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
                assert!(!std::str::from_utf8(&bytes).unwrap().contains("secret"));
            }
            let response = app
                .clone()
                .oneshot(request("POST", "/api/scan", &"x".repeat(4097)))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
            let response = app.oneshot(request("GET", "/api/scan", "")).await.unwrap();
            assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        })
    });
}

#[test]
fn streams_progress_then_a_complete_inventory_with_warnings_and_pinned_links() {
    let mut replies = snapshot();
    replies.extend([
        tree(
            ROOT,
            true,
            false,
            vec![skill("b/SKILL.md", B), skill(".hidden/SKILL.md", A)],
        ),
        blob(A, b"---\nname: '<img src=x onerror=alert(1)>'\n---\n"),
        blob(B, VALID),
    ]);
    with_web(replies, |app, runtime| {
        runtime.block_on(async {
            let events = events(app.oneshot(scan_request()).await.unwrap()).await;
            assert_eq!(events[0]["type"], "progress");
            assert_eq!(events[0]["message"], "Resolving repository: example/skills");
            assert!(events[..events.len() - 1]
                .iter()
                .all(|event| event["type"] == "progress"));
            assert!(events
                .iter()
                .any(|event| event["current"] == 2 && event["total"] == 2));
            let last = events.last().unwrap();
            assert_eq!(last["type"], "complete");
            let inventory = &last["inventory"];
            assert_eq!(inventory["repository"], "example/skills");
            assert_eq!(inventory["commit"], COMMIT);
            assert_eq!(inventory["aggregation"]["statistics"]["total_skills"], 2);
            assert_eq!(
                inventory["aggregation"]["statistics"]["standalone_skills"],
                2
            );
            assert_eq!(
                inventory["aggregation"]["statistics"]["skills_with_warnings"],
                1
            );
            assert_eq!(inventory["skills"][0]["path"], ".hidden/SKILL.md");
            assert_eq!(
                inventory["skills"][0]["name"],
                "<img src=x onerror=alert(1)>"
            );
            assert_eq!(
                inventory["skills"][0]["warnings"][0]["field"],
                "description"
            );
            assert_eq!(
                inventory["skills"][1]["link"],
                format!("https://github.com/example/skills/blob/{COMMIT}/b/SKILL.md")
            );
        })
    });
}

#[test]
fn includes_aggregated_groups_and_statistics_in_the_completed_scan() {
    let mut replies = snapshot();
    replies.extend([
        tree(
            ROOT,
            true,
            false,
            vec![
                skill("c/SKILL.md", C),
                skill("a/SKILL.md", A),
                skill("b/SKILL.md", B),
            ],
        ),
        blob(
            A,
            b"---\nname: Code Review\ndescription: Review changes\n---\n",
        ),
        blob(B, b"---\nname: code-review\n---\n"),
        blob(
            C,
            b"---\nname: release-notes\ndescription: Draft notes\n---\n",
        ),
    ]);
    with_web(replies, |app, runtime| {
        runtime.block_on(async {
            let events = events(app.oneshot(scan_request()).await.unwrap()).await;
            let inventory = &events.last().unwrap()["inventory"];
            assert_eq!(inventory["skills"].as_array().unwrap().len(), 3);
            assert_eq!(
                inventory["aggregation"],
                json!({
                    "statistics": {
                        "total_skills": 3,
                        "similar_groups": 1,
                        "grouped_skills": 2,
                        "standalone_skills": 1,
                        "largest_group": 2,
                        "skills_with_warnings": 1
                    },
                    "groups": [
                        {"skill_indices": [0, 1], "skills_with_warnings": 1},
                        {"skill_indices": [2], "skills_with_warnings": 0}
                    ]
                })
            );
        })
    });
}

#[test]
fn a_failed_download_sends_an_error_without_partial_inventory() {
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
            json!({ "message": "secret" }),
        ),
    ]);
    with_web(replies, |app, runtime| {
        runtime.block_on(async {
            let events = events(app.oneshot(scan_request()).await.unwrap()).await;
            assert_eq!(events.last().unwrap()["type"], "error");
            assert!(events.last().unwrap()["message"]
                .as_str()
                .unwrap()
                .contains("HTTP 404"));
            assert!(!events
                .iter()
                .any(|event| event["type"] == "complete" || event.get("inventory").is_some()));
            assert!(!serde_json::to_string(&events).unwrap().contains("secret"));
        })
    });
}

#[test]
fn distinguishes_no_skills_from_an_empty_repository_in_web_results() {
    for empty in [true, false] {
        let replies = if empty {
            vec![
                repository(),
                Reply::json(
                    "/repos/example/skills/commits/main",
                    409,
                    json!({"message":"Git Repository is empty."}),
                ),
            ]
        } else {
            let mut replies = snapshot();
            replies.push(tree(ROOT, true, false, vec![]));
            replies
        };
        with_web(replies, |app, runtime| {
            runtime.block_on(async {
                let events = events(app.oneshot(scan_request()).await.unwrap()).await;
                let last = events.last().unwrap();
                assert_eq!(last["type"], "complete");
                assert_eq!(last["inventory"]["skills"], json!([]));
                assert_eq!(last["inventory"]["commit"].is_null(), empty);
                assert_eq!(last["inventory"]["aggregation"]["groups"], json!([]));
                assert_eq!(
                    last["inventory"]["aggregation"]["statistics"],
                    json!({
                        "total_skills": 0, "similar_groups": 0, "grouped_skills": 0,
                        "standalone_skills": 0, "largest_group": 0, "skills_with_warnings": 0
                    })
                );
            })
        });
    }
}

#[test]
fn concurrent_scans_are_rejected_and_disconnect_releases_capacity_after_completion() {
    let directory = TestDirectory::new();
    let mut replies = snapshot();
    // More progress than the bounded channel can hold keeps the first worker
    // active until the body is consumed or dropped, without timing assumptions.
    replies.push(tree(
        ROOT,
        true,
        false,
        (0..10)
            .map(|n| skill(&format!("{n}/SKILL.md"), A))
            .collect(),
    ));
    replies.extend((0..10).map(|_| blob(A, VALID)));
    replies.extend(snapshot());
    with_web_storage(
        replies,
        crate::cache::ScanCache::new(directory.0.join("cache")),
        crate::recent::RecentRepositories::new(directory.0.join("recent")),
        |app, runtime| {
            runtime.block_on(async {
                let first = app.clone().oneshot(scan_request()).await.unwrap();
                assert_eq!(first.status(), StatusCode::OK);
                let second = app.clone().oneshot(scan_request()).await.unwrap();
                assert_eq!(second.status(), StatusCode::CONFLICT);
                drop(first);
                // Finishing already-started GitHub work after disconnection is intentional.
                let response = tokio::time::timeout(Duration::from_secs(5), async {
                    loop {
                        let response = app.clone().oneshot(scan_request()).await.unwrap();
                        if response.status() != StatusCode::CONFLICT {
                            break response;
                        }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                })
                .await
                .unwrap();
                // The disconnected scan saved its summary before releasing capacity.
                assert_eq!(
                    recent_list(&app).await["repositories"][0]["skill_count"],
                    10
                );
                let events = events(response).await;
                assert_eq!(events.last().unwrap()["type"], "complete");
                assert_eq!(
                    events.last().unwrap()["inventory"]["skills"]
                        .as_array()
                        .unwrap()
                        .len(),
                    10
                );
                assert!(events
                    .iter()
                    .any(|event| event["message"].as_str().is_some_and(
                        |message| message.starts_with("Using cached analysis for commit:")
                    )));
            })
        },
    );
}

#[test]
fn cli_and_web_share_commit_validated_analysis_across_cache_instances() {
    use crate::cache::ScanCache;

    let directory = TestDirectory::new();
    let mut replies = snapshot();
    replies.extend([
        tree(ROOT, true, false, vec![skill("SKILL.md", A)]),
        blob(A, b"missing metadata"),
    ]);
    replies.extend(snapshot());
    replies.push(repository());
    replies.push(Reply::json(
        "/repos/example/skills/commits/main",
        200,
        json!({"sha": D, "commit": {"tree": {"sha": ROOT}}}),
    ));
    replies.extend([
        tree(ROOT, true, false, vec![skill("new/SKILL.md", B)]),
        blob(B, VALID),
    ]);
    replies.push(repository());
    replies.push(Reply::json(
        "/repos/example/skills/commits/main",
        200,
        json!({"sha": D, "commit": {"tree": {"sha": ROOT}}}),
    ));
    let server = Server::start(replies);
    let client = Arc::new(server.client());
    let repository = "example/skills".parse().unwrap();
    let cli_inventory = scanner::scan_with_cache(
        &client,
        &repository,
        &ScanCache::new(directory.0.clone()),
        |_| {},
    )
    .unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let refreshed = runtime.block_on(async {
        let app = crate::web::router(
            client.clone(),
            3000,
            ScanCache::new(directory.0.clone()),
            crate::recent::RecentRepositories::disabled(),
            crate::starred::StarredSkills::disabled(),
            crate::codex::CodexSkills::disabled(),
        );
        let cached = events(app.clone().oneshot(scan_request()).await.unwrap()).await;
        assert_eq!(cached.len(), 4);
        assert_eq!(
            cached[2]["message"],
            format!("Using cached analysis for commit: {COMMIT}")
        );
        assert!(cached[2]["current"].is_null());
        assert!(cached[2]["total"].is_null());
        assert!(cached[..3]
            .iter()
            .all(|event| event.get("inventory").is_none()));
        assert_eq!(
            cached[3]["inventory"]["skills"],
            serde_json::to_value(&cli_inventory.skills).unwrap()
        );
        assert_eq!(
            cached[3]["inventory"]["aggregation"],
            serde_json::to_value(crate::aggregation::aggregate(&cli_inventory.skills)).unwrap()
        );
        let refreshed = events(app.oneshot(scan_request()).await.unwrap()).await;
        assert!(refreshed.iter().any(|event| event["current"] == 1));
        let inventory = refreshed.last().unwrap()["inventory"].clone();
        assert_eq!(inventory["commit"], D);
        assert_eq!(inventory["skills"][0]["path"], "new/SKILL.md");
        inventory
    });
    drop(runtime);
    let cli_inventory = scanner::scan_with_cache(
        &client,
        &repository,
        &ScanCache::new(directory.0.clone()),
        |_| {},
    )
    .unwrap();
    assert_eq!(
        refreshed["skills"],
        serde_json::to_value(&cli_inventory.skills).unwrap()
    );
    assert_eq!(cli_inventory.commit.as_deref(), Some(D));
    assert_eq!(server.finish().len(), 12);
}

async fn recent_list(app: &Router) -> Value {
    let response = app
        .clone()
        .oneshot(request("GET", "/api/recent", ""))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap()
}

#[test]
fn recent_api_shares_disk_state_records_scans_and_persists_removal() {
    use crate::{cache::ScanCache, recent::RecentRepositories};
    let directory = TestDirectory::new();
    let recent = RecentRepositories::new(directory.0.clone());
    // A CLI scan and web server use independent store instances.
    let mut replies = snapshot();
    replies.push(tree(ROOT, true, false, vec![]));
    let server = Server::start(replies);
    scanner::scan_with_storage(
        &server.client(),
        &"example/skills".parse().unwrap(),
        &ScanCache::disabled(),
        &recent,
        |_| {},
    )
    .unwrap();
    server.finish();
    let mut replies = snapshot();
    replies.extend([
        tree(ROOT, true, false, vec![skill("SKILL.md", A)]),
        blob(A, VALID),
    ]);
    with_web_storage(
        replies,
        ScanCache::disabled(),
        RecentRepositories::new(directory.0.clone()),
        |app, runtime| {
            runtime.block_on(async {
                let list = recent_list(&app).await;
                assert_eq!(list["enabled"], true);
                assert_eq!(list["repositories"][0]["repository"], "example/skills");
                assert_eq!(list["repositories"][0]["skill_count"], 0);
                let scan = events(app.clone().oneshot(scan_request()).await.unwrap()).await;
                assert_eq!(scan.last().unwrap()["type"], "complete");
                assert_eq!(recent_list(&app).await["repositories"][0]["skill_count"], 1);
                assert_eq!(recent.list().unwrap().len(), 1);
                for _ in 0..2 {
                    let response = app
                        .clone()
                        .oneshot(request(
                            "POST",
                            "/api/recent/remove",
                            r#"{"repository":"https://github.com/EXAMPLE/Skills.git/"}"#,
                        ))
                        .await
                        .unwrap();
                    assert_eq!(response.status(), StatusCode::NO_CONTENT);
                }
                assert_eq!(recent_list(&app).await["repositories"], json!([]));
            });
        },
    );
    with_web_storage(
        vec![],
        ScanCache::disabled(),
        RecentRepositories::new(directory.0.clone()),
        |app, runtime| {
            runtime.block_on(async {
                assert_eq!(recent_list(&app).await["repositories"], json!([]));
            });
        },
    );
    assert!(recent.list().unwrap().is_empty());
}

#[test]
fn recent_removal_has_the_same_input_and_browser_protections_as_scan() {
    with_web(vec![], |app, runtime| {
        runtime.block_on(async {
            assert_eq!(
                recent_list(&app).await,
                json!({"enabled":false,"repositories":[]})
            );
            let valid = r#"{"repository":"example/skills"}"#;
            let mut missing_header = request("POST", "/api/recent/remove", valid);
            missing_header.headers_mut().remove("x-skill-scanner");
            let mut foreign = request("POST", "/api/recent/remove", valid);
            foreign
                .headers_mut()
                .insert("origin", "https://foreign.example".parse().unwrap());
            let mut foreign_host = request("GET", "/api/recent", "");
            foreign_host
                .headers_mut()
                .insert("host", "foreign.example:3000".parse().unwrap());
            for request in [missing_header, foreign, foreign_host] {
                assert_eq!(
                    app.clone().oneshot(request).await.unwrap().status(),
                    StatusCode::FORBIDDEN
                );
            }
            for (body, status) in [
                (
                    r#"{"repository":"https://user:secret@github.com/a/b"}"#,
                    StatusCode::BAD_REQUEST,
                ),
                (r#"{"repository":"../escape"}"#, StatusCode::BAD_REQUEST),
                (
                    r#"{"repository":"a/b","unknown":true}"#,
                    StatusCode::UNPROCESSABLE_ENTITY,
                ),
                ("{}", StatusCode::UNPROCESSABLE_ENTITY),
                ("broken", StatusCode::BAD_REQUEST),
            ] {
                let response = app
                    .clone()
                    .oneshot(request("POST", "/api/recent/remove", body))
                    .await
                    .unwrap();
                assert_eq!(response.status(), status);
                let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
                assert!(!std::str::from_utf8(&bytes).unwrap().contains("secret"));
            }
            assert_eq!(
                app.clone()
                    .oneshot(request("POST", "/api/recent/remove", &"x".repeat(4097)))
                    .await
                    .unwrap()
                    .status(),
                StatusCode::PAYLOAD_TOO_LARGE
            );
            let mut missing_content_type = request("POST", "/api/recent/remove", valid);
            missing_content_type.headers_mut().remove("content-type");
            assert_eq!(
                app.oneshot(missing_content_type).await.unwrap().status(),
                StatusCode::UNSUPPORTED_MEDIA_TYPE
            );
        });
    });
}

#[test]
fn unavailable_history_returns_errors_and_a_warning_without_losing_scan_results() {
    let directory = TestDirectory::new();
    let path = directory.0.join("file");
    std::fs::write(&path, "not a directory").unwrap();
    let mut replies = snapshot();
    replies.push(tree(ROOT, true, false, vec![]));
    with_web_storage(
        replies,
        crate::cache::ScanCache::disabled(),
        crate::recent::RecentRepositories::new(path),
        |app, runtime| {
            runtime.block_on(async {
                for (method, path, body) in [
                    ("GET", "/api/recent", ""),
                    (
                        "POST",
                        "/api/recent/remove",
                        r#"{"repository":"example/skills"}"#,
                    ),
                ] {
                    assert_eq!(
                        app.clone()
                            .oneshot(request(method, path, body))
                            .await
                            .unwrap()
                            .status(),
                        StatusCode::INTERNAL_SERVER_ERROR
                    );
                }
                let events = events(app.oneshot(scan_request()).await.unwrap()).await;
                assert!(events
                    .iter()
                    .any(|event| event["type"] == "history_warning"));
                assert_eq!(events.last().unwrap()["type"], "complete");
            });
        },
    );
}

fn organization_request(body: &str) -> Request<Body> {
    request("POST", "/api/scan-org", body)
}

#[test]
fn organization_scan_streams_repository_progress_then_one_combined_inventory() {
    use super::organization_scan::{commit, listed, listing, recursive_tree, repository_blob};
    let replies = vec![
        listing(1, vec![listed("tools"), listed("broken")]),
        commit("broken"),
        recursive_tree("broken", true, vec![]),
        commit("tools"),
        recursive_tree("tools", false, vec![skill("review/SKILL.md", A)]),
        repository_blob("tools", A, VALID),
    ];
    with_web(replies, |app, runtime| {
        runtime.block_on(async {
            let response = app
                .oneshot(organization_request(r#"{"organization":" example "}"#))
                .await
                .unwrap();
            let events = events(response).await;
            assert_eq!(
                events[0],
                json!({"type": "progress", "message": "Listing public repositories: example (page 1)", "current": null, "total": null})
            );
            assert_eq!(
                events[1],
                json!({"type": "progress", "message": "Scanning repository [1/2]: example/broken", "current": 1, "total": 2})
            );
            assert_eq!(
                events[2]["message"],
                "example/broken: Resolving default branch: main"
            );
            assert_eq!(events[2]["current"], 1);
            // Repository counters, not nested skill counters, drive the progress bar.
            assert!(events.iter().any(|event| event["message"]
                == "example/tools: Scanning skill [1/1]: review/SKILL.md"
                && event["current"] == 2
                && event["total"] == 2));
            let last = events.last().unwrap();
            assert_eq!(last["type"], "complete");
            let inventory = &last["inventory"];
            assert_eq!(inventory["organization"], "example");
            assert_eq!(inventory["skipped_forks"], 0);
            assert!(inventory.get("repository").is_none());
            assert_eq!(inventory["repositories"][0]["repository"], "example/broken");
            assert_eq!(inventory["repositories"][0]["commit"], Value::Null);
            assert!(inventory["repositories"][0]["error"]
                .as_str()
                .unwrap()
                .contains("scan this repository individually"));
            assert_eq!(
                inventory["repositories"][1],
                json!({"repository": "example/tools", "commit": COMMIT, "skill_count": 1, "error": null})
            );
            assert_eq!(
                inventory["skills"][0]["path"],
                "example/tools/review/SKILL.md"
            );
            assert_eq!(
                inventory["skills"][0]["link"],
                format!("https://github.com/example/tools/blob/{COMMIT}/review/SKILL.md")
            );
            assert_eq!(inventory["aggregation"]["statistics"]["total_skills"], 1);
            assert_eq!(
                inventory["aggregation"]["groups"][0]["skill_indices"],
                json!([0])
            );
        })
    });
}

#[test]
fn organization_scans_validate_input_without_contacting_github_or_echoing_secrets() {
    with_web(vec![], |app, runtime| {
        runtime.block_on(async {
            for (body, status) in [
                (
                    r#"{"organization":"example/skills"}"#,
                    StatusCode::BAD_REQUEST,
                ),
                (
                    r#"{"organization":"https://name:secret@github.com/example"}"#,
                    StatusCode::BAD_REQUEST,
                ),
                ("{invalid", StatusCode::BAD_REQUEST),
                (
                    r#"{"repository":"example"}"#,
                    StatusCode::UNPROCESSABLE_ENTITY,
                ),
                (
                    r#"{"organization":"example","token":"secret"}"#,
                    StatusCode::UNPROCESSABLE_ENTITY,
                ),
            ] {
                let response = app
                    .clone()
                    .oneshot(organization_request(body))
                    .await
                    .unwrap();
                assert_eq!(response.status(), status, "{body}");
                let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
                assert!(!std::str::from_utf8(&bytes).unwrap().contains("secret"));
            }
            let unmarked = Request::builder()
                .method("POST")
                .uri("/api/scan-org")
                .header("host", HOST)
                .header("content-type", "application/json")
                .body(Body::from(r#"{"organization":"example"}"#))
                .unwrap();
            let response = app.clone().oneshot(unmarked).await.unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
            let response = app
                .oneshot(organization_request(&"x".repeat(4097)))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        })
    });
}

#[test]
fn repository_and_organization_scans_share_one_scan_at_a_time() {
    let mut replies = snapshot();
    // Enough progress to fill the bounded channel keeps the first scan running.
    replies.push(tree(
        ROOT,
        true,
        false,
        (0..10)
            .map(|n| skill(&format!("{n}/SKILL.md"), A))
            .collect(),
    ));
    replies.extend((0..10).map(|_| blob(A, VALID)));
    replies.push(super::organization_scan::listing(1, vec![]));
    with_web(replies, |app, runtime| {
        runtime.block_on(async {
            let body = r#"{"organization":"example"}"#;
            let first = app.clone().oneshot(scan_request()).await.unwrap();
            assert_eq!(first.status(), StatusCode::OK);
            let second = app
                .clone()
                .oneshot(organization_request(body))
                .await
                .unwrap();
            assert_eq!(second.status(), StatusCode::CONFLICT);
            drop(first);
            let response = tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    let response = app
                        .clone()
                        .oneshot(organization_request(body))
                        .await
                        .unwrap();
                    if response.status() != StatusCode::CONFLICT {
                        break response;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            let events = events(response).await;
            let inventory = &events.last().unwrap()["inventory"];
            assert_eq!(inventory["repositories"], json!([]));
            assert_eq!(inventory["aggregation"]["statistics"]["total_skills"], 0);
        })
    });
}

async fn starred_list(app: &Router) -> Value {
    let response = app
        .clone()
        .oneshot(request("GET", "/api/starred", ""))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap()
}

async fn status(app: &Router, method: &str, path: &str, body: &str) -> StatusCode {
    app.clone()
        .oneshot(request(method, path, body))
        .await
        .unwrap()
        .status()
}

#[test]
fn starred_api_persists_stars_across_servers_and_the_library() {
    use crate::starred::StarredSkills;
    let directory = TestDirectory::new();
    let star = json!({
        "repository": "https://github.com/Example/Skills",
        "path": "skills/review/SKILL.md",
        "name": "review",
        "commit": COMMIT,
    })
    .to_string();
    with_web_stores(
        vec![],
        crate::cache::ScanCache::disabled(),
        crate::recent::RecentRepositories::disabled(),
        StarredSkills::new(directory.0.clone()),
        |app, runtime| {
            runtime.block_on(async {
                assert_eq!(
                    starred_list(&app).await,
                    json!({"enabled": true, "skills": []})
                );
                for _ in 0..2 {
                    assert_eq!(
                        status(&app, "POST", "/api/starred/add", &star).await,
                        StatusCode::NO_CONTENT
                    );
                }
                let list = starred_list(&app).await;
                assert_eq!(list["skills"].as_array().unwrap().len(), 1);
                let skill = &list["skills"][0];
                assert_eq!(skill["repository"], "Example/Skills");
                assert_eq!(skill["path"], "skills/review/SKILL.md");
                assert_eq!(skill["name"], "review");
                assert_eq!(skill["commit"], COMMIT);
                assert_eq!(
                    skill["link"],
                    format!(
                        "https://github.com/Example/Skills/blob/{COMMIT}/skills/review/SKILL.md"
                    )
                );
                assert!(skill["starred_at"].as_u64().unwrap() > 0);
            });
        },
    );
    // A new server and the CLI's store instance see the same disk state.
    assert_eq!(
        StarredSkills::new(directory.0.clone()).list().unwrap()[0].name,
        "review"
    );
    with_web_stores(
        vec![],
        crate::cache::ScanCache::disabled(),
        crate::recent::RecentRepositories::disabled(),
        StarredSkills::new(directory.0.clone()),
        |app, runtime| {
            runtime.block_on(async {
                assert_eq!(starred_list(&app).await["skills"][0]["name"], "review");
                for _ in 0..2 {
                    assert_eq!(
                        status(
                            &app,
                            "POST",
                            "/api/starred/remove",
                            r#"{"repository":"example/skills","path":"skills/review/SKILL.md"}"#,
                        )
                        .await,
                        StatusCode::NO_CONTENT
                    );
                }
                assert_eq!(starred_list(&app).await["skills"], json!([]));
            });
        },
    );
    assert!(StarredSkills::new(directory.0.clone())
        .list()
        .unwrap()
        .is_empty());
}

#[test]
fn starred_routes_validate_input_and_have_the_same_browser_protections_as_scan() {
    let directory = TestDirectory::new();
    with_web_stores(
        vec![],
        crate::cache::ScanCache::disabled(),
        crate::recent::RecentRepositories::disabled(),
        crate::starred::StarredSkills::new(directory.0.clone()),
        |app, runtime| {
            runtime.block_on(async {
                let valid = json!({
                    "repository": "example/skills",
                    "path": "SKILL.md",
                    "name": "skills",
                    "commit": COMMIT,
                });
                let with = |field: &str, value: Value| {
                    let mut body = valid.clone();
                    body[field] = value;
                    body.to_string()
                };
                let mut missing_header = request("POST", "/api/starred/add", &valid.to_string());
                missing_header.headers_mut().remove("x-skill-scanner");
                let mut foreign = request("POST", "/api/starred/remove", &valid.to_string());
                foreign
                    .headers_mut()
                    .insert("origin", "https://foreign.example".parse().unwrap());
                let mut foreign_host = request("GET", "/api/starred", "");
                foreign_host
                    .headers_mut()
                    .insert("host", "foreign.example:3000".parse().unwrap());
                for request in [missing_header, foreign, foreign_host] {
                    assert_eq!(
                        app.clone().oneshot(request).await.unwrap().status(),
                        StatusCode::FORBIDDEN
                    );
                }
                for (path, body, status) in [
                    (
                        "/api/starred/add",
                        with("repository", json!("https://user:secret@github.com/a/b")),
                        StatusCode::BAD_REQUEST,
                    ),
                    (
                        "/api/starred/add",
                        with("path", json!("../SKILL.md")),
                        StatusCode::BAD_REQUEST,
                    ),
                    (
                        "/api/starred/add",
                        with("path", json!("README.md")),
                        StatusCode::BAD_REQUEST,
                    ),
                    (
                        "/api/starred/add",
                        with("commit", json!("main")),
                        StatusCode::BAD_REQUEST,
                    ),
                    (
                        "/api/starred/add",
                        with("name", json!("")),
                        StatusCode::BAD_REQUEST,
                    ),
                    (
                        "/api/starred/add",
                        with("unknown", json!(true)),
                        StatusCode::UNPROCESSABLE_ENTITY,
                    ),
                    (
                        "/api/starred/add",
                        r#"{"repository":"a/b","path":"SKILL.md"}"#.into(),
                        StatusCode::UNPROCESSABLE_ENTITY,
                    ),
                    ("/api/starred/add", "broken".into(), StatusCode::BAD_REQUEST),
                    (
                        "/api/starred/remove",
                        r#"{"repository":"a/b","path":"x//SKILL.md"}"#.into(),
                        StatusCode::BAD_REQUEST,
                    ),
                    (
                        "/api/starred/remove",
                        valid.to_string(),
                        StatusCode::UNPROCESSABLE_ENTITY,
                    ),
                    (
                        "/api/starred/remove",
                        r#"{"repository":"a/b"}"#.into(),
                        StatusCode::UNPROCESSABLE_ENTITY,
                    ),
                ] {
                    let response = app
                        .clone()
                        .oneshot(request("POST", path, &body))
                        .await
                        .unwrap();
                    assert_eq!(response.status(), status, "{path} {body}");
                    let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
                    assert!(!std::str::from_utf8(&bytes).unwrap().contains("secret"));
                }
                assert_eq!(
                    status(&app, "POST", "/api/starred/add", &"x".repeat(4097)).await,
                    StatusCode::PAYLOAD_TOO_LARGE
                );
                let mut missing_content_type =
                    request("POST", "/api/starred/add", &valid.to_string());
                missing_content_type.headers_mut().remove("content-type");
                assert_eq!(
                    app.clone()
                        .oneshot(missing_content_type)
                        .await
                        .unwrap()
                        .status(),
                    StatusCode::UNSUPPORTED_MEDIA_TYPE
                );
                assert_eq!(
                    status(&app, "GET", "/api/starred/add", "").await,
                    StatusCode::METHOD_NOT_ALLOWED
                );
                assert_eq!(starred_list(&app).await["skills"], json!([]));
            });
        },
    );
    assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 0);
}

#[test]
fn disabled_and_unavailable_starred_skills_are_reported_without_affecting_scans() {
    let star = json!({
        "repository": "example/skills",
        "path": "SKILL.md",
        "name": "skills",
        "commit": COMMIT,
    })
    .to_string();
    let unstar = r#"{"repository":"example/skills","path":"SKILL.md"}"#;
    with_web(vec![], |app, runtime| {
        runtime.block_on(async {
            assert_eq!(
                starred_list(&app).await,
                json!({"enabled": false, "skills": []})
            );
            assert_eq!(
                status(&app, "POST", "/api/starred/add", &star).await,
                StatusCode::CONFLICT
            );
            assert_eq!(
                status(&app, "POST", "/api/starred/remove", unstar).await,
                StatusCode::NO_CONTENT
            );
        });
    });
    let directory = TestDirectory::new();
    let path = directory.0.join("file");
    std::fs::write(&path, "not a directory").unwrap();
    let mut replies = snapshot();
    replies.push(tree(ROOT, true, false, vec![]));
    with_web_stores(
        replies,
        crate::cache::ScanCache::disabled(),
        crate::recent::RecentRepositories::disabled(),
        crate::starred::StarredSkills::new(path.clone()),
        |app, runtime| {
            runtime.block_on(async {
                for (method, route, body) in [
                    ("GET", "/api/starred", ""),
                    ("POST", "/api/starred/add", star.as_str()),
                    ("POST", "/api/starred/remove", unstar),
                ] {
                    assert_eq!(
                        status(&app, method, route, body).await,
                        StatusCode::INTERNAL_SERVER_ERROR
                    );
                }
                let events = events(app.oneshot(scan_request()).await.unwrap()).await;
                assert_eq!(events.last().unwrap()["type"], "complete");
            });
        },
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), "not a directory");
}

fn with_codex(
    replies: Vec<Reply>,
    codex: crate::codex::CodexSkills,
    test: impl FnOnce(Router, &tokio::runtime::Runtime),
) {
    with_web_codex(
        replies,
        crate::cache::ScanCache::disabled(),
        crate::recent::RecentRepositories::disabled(),
        crate::starred::StarredSkills::disabled(),
        codex,
        test,
    );
}

async fn codex_response(app: &Router, method: &str, path: &str, body: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(request(method, path, body))
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn install_body(repository: &str, path: &str) -> String {
    json!({ "repository": repository, "path": path, "commit": COMMIT }).to_string()
}

/// GitHub responses for installing skills/code-review at the pinned commit.
fn code_review_install() -> Vec<Reply> {
    vec![
        repository(),
        Reply::json(
            format!("/repos/example/skills/commits/{COMMIT}"),
            200,
            json!({ "sha": COMMIT, "commit": { "tree": { "sha": ROOT } } }),
        ),
        tree(
            ROOT,
            false,
            false,
            vec![entry("skills", A, "040000", "tree", None)],
        ),
        tree(
            A,
            false,
            false,
            vec![entry("code-review", B, "040000", "tree", None)],
        ),
        tree(B, true, false, vec![skill("SKILL.md", C)]),
        blob(C, VALID),
    ]
}

#[test]
fn codex_api_installs_the_scanned_commit_and_shares_installations_with_the_library() {
    use crate::codex::CodexSkills;
    let directory = TestDirectory::new();
    with_codex(
        code_review_install(),
        CodexSkills::new(directory.0.clone()),
        |app, runtime| {
            runtime.block_on(async {
                assert_eq!(
                    codex_response(&app, "GET", "/api/codex", "").await,
                    (
                        StatusCode::OK,
                        json!({
                            "enabled": true,
                            "directory": directory.0.to_string_lossy(),
                            "skills": [],
                        })
                    )
                );
                let (status, installed) = codex_response(
                    &app,
                    "POST",
                    "/api/codex/install",
                    &install_body("https://github.com/example/skills", "skills/code-review/SKILL.md"),
                )
                .await;
                assert_eq!(status, StatusCode::OK);
                assert_eq!(installed["files"], 1);
                let skill = &installed["skill"];
                assert_eq!(skill["name"], "code-review");
                assert_eq!(skill["repository"], "example/skills");
                assert_eq!(skill["path"], "skills/code-review/SKILL.md");
                assert_eq!(skill["commit"], COMMIT);
                assert_eq!(
                    skill["link"],
                    format!("https://github.com/example/skills/blob/{COMMIT}/skills/code-review/SKILL.md")
                );
                assert!(skill["installed_at"].as_u64().unwrap() > 0);
                let (_, list) = codex_response(&app, "GET", "/api/codex", "").await;
                assert_eq!(list["skills"], json!([skill]));
                // Another repository's skill with the same folder name conflicts
                // without contacting GitHub.
                let (status, conflict) = codex_response(
                    &app,
                    "POST",
                    "/api/codex/install",
                    &install_body("other/tools", "code-review/SKILL.md"),
                )
                .await;
                assert_eq!(status, StatusCode::CONFLICT);
                assert!(conflict["message"]
                    .as_str()
                    .unwrap()
                    .contains("already holds skills/code-review/SKILL.md from example/skills"));
            });
        },
    );
    assert_eq!(
        std::fs::read(directory.0.join("code-review/SKILL.md")).unwrap(),
        VALID
    );
    // A new server and the CLI's library instance see the same installation.
    assert_eq!(
        CodexSkills::new(directory.0.clone()).list().unwrap()[0].name,
        "code-review"
    );
    with_codex(
        vec![],
        CodexSkills::new(directory.0.clone()),
        |app, runtime| {
            runtime.block_on(async {
                for _ in 0..2 {
                    assert_eq!(
                        status(
                            &app,
                            "POST",
                            "/api/codex/remove",
                            r#"{"name":"code-review"}"#
                        )
                        .await,
                        StatusCode::NO_CONTENT
                    );
                }
                let (_, list) = codex_response(&app, "GET", "/api/codex", "").await;
                assert_eq!(list["skills"], json!([]));
            });
        },
    );
    assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 0);
}

#[test]
fn codex_routes_validate_input_and_have_the_same_browser_protections_as_scan() {
    let directory = TestDirectory::new();
    std::fs::create_dir(directory.0.join("mine")).unwrap();
    with_codex(
        vec![Reply::json(
            "/repos/example/skills",
            404,
            json!({ "message": "secret-detail" }),
        )],
        crate::codex::CodexSkills::new(directory.0.clone()),
        |app, runtime| {
            runtime.block_on(async {
                let valid = install_body("example/skills", "skills/code-review/SKILL.md");
                let mut missing_header = request("POST", "/api/codex/install", &valid);
                missing_header.headers_mut().remove("x-skill-scanner");
                let mut foreign = request("POST", "/api/codex/remove", r#"{"name":"mine"}"#);
                foreign
                    .headers_mut()
                    .insert("origin", "https://foreign.example".parse().unwrap());
                let mut foreign_host = request("GET", "/api/codex", "");
                foreign_host
                    .headers_mut()
                    .insert("host", "foreign.example:3000".parse().unwrap());
                for request in [missing_header, foreign, foreign_host] {
                    assert_eq!(
                        app.clone().oneshot(request).await.unwrap().status(),
                        StatusCode::FORBIDDEN
                    );
                }
                let oversized = install_body("example/skills", &format!("{}/SKILL.md", "a".repeat(5000)));
                for (route, body, expected) in [
                    ("/api/codex/install", "{", StatusCode::BAD_REQUEST),
                    ("/api/codex/install", r#"{"repository":"example/skills"}"#, StatusCode::UNPROCESSABLE_ENTITY),
                    ("/api/codex/install", &oversized, StatusCode::PAYLOAD_TOO_LARGE),
                    ("/api/codex/install", &install_body("https://user:secret@github.com/a/b", "SKILL.md"), StatusCode::BAD_REQUEST),
                    ("/api/codex/install", &install_body("example/skills", "README.md"), StatusCode::BAD_REQUEST),
                    ("/api/codex/install", &install_body("example/skills", ".hidden/SKILL.md"), StatusCode::BAD_REQUEST),
                    ("/api/codex/install", &json!({"repository": "example/skills", "path": "SKILL.md", "commit": "main"}).to_string(), StatusCode::BAD_REQUEST),
                    ("/api/codex/install", &install_body("example/tools", "mine/SKILL.md"), StatusCode::CONFLICT),
                    ("/api/codex/remove", r#"{"name":"../escape"}"#, StatusCode::BAD_REQUEST),
                    ("/api/codex/remove", r#"{"name":"mine","extra":1}"#, StatusCode::UNPROCESSABLE_ENTITY),
                    ("/api/codex/remove", r#"{"name":"mine"}"#, StatusCode::CONFLICT),
                ] {
                    let (status, body) = codex_response(&app, "POST", route, body).await;
                    assert_eq!(status, expected, "{route} {body}");
                    assert!(!body.to_string().contains("secret"));
                }
                // GitHub failures are reported without server error text.
                let (status, body) = codex_response(&app, "POST", "/api/codex/install", &valid).await;
                assert_eq!(status, StatusCode::BAD_GATEWAY);
                let message = body["message"].as_str().unwrap();
                assert!(message.contains("HTTP 404"));
                assert!(!message.contains("secret-detail"));
            });
        },
    );
    assert!(directory.0.join("mine").exists());
    assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn one_codex_change_runs_at_a_time_independently_of_scans() {
    let directory = TestDirectory::new();
    let mut slow = Reply::json("/repos/example/skills", 404, json!({}));
    slow.delay = Duration::from_millis(1500);
    let mut replies = vec![slow];
    replies.extend(snapshot());
    replies.push(tree(ROOT, true, false, vec![]));
    with_codex(
        replies,
        crate::codex::CodexSkills::new(directory.0.clone()),
        |app, runtime| {
            runtime.block_on(async {
                let body = install_body("example/skills", "skills/code-review/SKILL.md");
                let first = tokio::spawn(app.clone().oneshot(request(
                    "POST",
                    "/api/codex/install",
                    &body,
                )));
                tokio::time::sleep(Duration::from_millis(300)).await;
                for (route, body) in [
                    ("/api/codex/install", body.as_str()),
                    ("/api/codex/remove", r#"{"name":"code-review"}"#),
                ] {
                    let (status, response) = codex_response(&app, "POST", route, body).await;
                    assert_eq!(status, StatusCode::CONFLICT);
                    assert!(response["message"]
                        .as_str()
                        .unwrap()
                        .contains("Another Codex skill change"));
                }
                // Scans keep their own limit while an installation runs.
                let events = events(app.clone().oneshot(scan_request()).await.unwrap()).await;
                assert_eq!(events.last().unwrap()["type"], "complete");
                assert_eq!(
                    first.await.unwrap().unwrap().status(),
                    StatusCode::BAD_GATEWAY
                );
                assert_eq!(
                    status(
                        &app,
                        "POST",
                        "/api/codex/remove",
                        r#"{"name":"code-review"}"#
                    )
                    .await,
                    StatusCode::NO_CONTENT
                );
            });
        },
    );
}

#[test]
fn disabled_and_unavailable_codex_skills_are_reported_without_affecting_scans() {
    let install = install_body("example/skills", "skills/code-review/SKILL.md");
    with_web(vec![], |app, runtime| {
        runtime.block_on(async {
            assert_eq!(
                codex_response(&app, "GET", "/api/codex", "").await,
                (
                    StatusCode::OK,
                    json!({"enabled": false, "directory": null, "skills": []})
                )
            );
            for (route, body) in [
                ("/api/codex/install", install.as_str()),
                ("/api/codex/remove", r#"{"name":"code-review"}"#),
            ] {
                let (status, response) = codex_response(&app, "POST", route, body).await;
                assert_eq!(status, StatusCode::CONFLICT);
                assert!(response["message"].as_str().unwrap().contains("disabled"));
            }
        });
    });
    let directory = TestDirectory::new();
    let path = directory.0.join("file");
    std::fs::write(&path, "not a directory").unwrap();
    let mut replies = snapshot();
    replies.push(tree(ROOT, true, false, vec![]));
    with_codex(
        replies,
        crate::codex::CodexSkills::new(path.clone()),
        |app, runtime| {
            runtime.block_on(async {
                for (method, route, body) in [
                    ("GET", "/api/codex", ""),
                    ("POST", "/api/codex/install", install.as_str()),
                    ("POST", "/api/codex/remove", r#"{"name":"code-review"}"#),
                ] {
                    assert_eq!(
                        status(&app, method, route, body).await,
                        StatusCode::INTERNAL_SERVER_ERROR
                    );
                }
                let events = events(app.oneshot(scan_request()).await.unwrap()).await;
                assert_eq!(events.last().unwrap()["type"], "complete");
            });
        },
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), "not a directory");
}
