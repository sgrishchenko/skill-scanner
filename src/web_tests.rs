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
    let server = Server::start(replies);
    // The blocking reqwest client must be created and dropped outside Tokio.
    let client = Arc::new(server.client());
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    test(crate::web::router(client.clone(), 3000), &runtime);
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
        let app = crate::web::router(client.clone(), 80);
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
            })
        });
    }
}

#[test]
fn concurrent_scans_are_rejected_and_disconnect_releases_capacity_after_completion() {
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
    replies.push(tree(ROOT, true, false, vec![]));
    with_web(replies, |app, runtime| {
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
            let events = events(response).await;
            assert_eq!(events.last().unwrap()["type"], "complete");
        })
    });
}
