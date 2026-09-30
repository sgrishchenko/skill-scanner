use std::{convert::Infallible, io, net::Ipv4Addr, sync::Arc};

use axum::{
    body::Body,
    extract::{rejection::JsonRejection, DefaultBodyLimit, Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Semaphore};
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    aggregation::{self, Aggregation},
    github::GitHubClient,
    report,
    repository::Repository,
    scanner::{self, Inventory, ScanProgress, Skill},
};

const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

#[derive(Clone)]
struct WebState {
    client: Arc<GitHubClient>,
    available: Arc<Semaphore>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScanRequest {
    repository: String,
}

#[derive(Serialize)]
struct WebInventory {
    repository: String,
    commit: Option<String>,
    skills: Vec<Skill>,
    aggregation: Aggregation,
}

impl From<Inventory> for WebInventory {
    fn from(inventory: Inventory) -> Self {
        Self {
            aggregation: aggregation::aggregate(&inventory.skills),
            repository: inventory.repository.to_string(),
            commit: inventory.commit,
            skills: inventory.skills,
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Event {
    Progress {
        message: String,
        current: Option<usize>,
        total: Option<usize>,
    },
    Complete {
        inventory: WebInventory,
    },
    Error {
        message: String,
    },
}

/// Assets are embedded so a release executable needs no adjacent web directory.
/// Keep the blocking GitHub client outside the async runtime, including its drop.
pub fn serve(port: u16, client: GitHubClient) -> io::Result<()> {
    let client = Arc::new(client);
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("could not listen on 127.0.0.1:{port}: {error}. Try another --port."),
        )
    })?;
    listener.set_nonblocking(true)?;
    let port = listener.local_addr()?.port();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    eprintln!("Skill Scanner is available at http://127.0.0.1:{port}\nPress Ctrl+C to stop.");
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::from_std(listener)?;
        axum::serve(listener, router(client.clone(), port)).await
    })
}

pub(crate) fn router(client: Arc<GitHubClient>, port: u16) -> Router {
    Router::new()
        .route(
            "/",
            get(|| async {
                asset(
                    "text/html; charset=utf-8",
                    include_str!("../web/index.html"),
                )
            }),
        )
        .route(
            "/app.css",
            get(|| async { asset("text/css; charset=utf-8", include_str!("../web/app.css")) }),
        )
        .route(
            "/app.js",
            get(|| async {
                asset(
                    "text/javascript; charset=utf-8",
                    include_str!("../web/app.js"),
                )
            }),
        )
        .route(
            "/favicon.svg",
            get(|| async { asset("image/svg+xml", include_str!("../web/favicon.svg")) }),
        )
        .route("/api/scan", post(scan))
        .fallback(|| async { error(StatusCode::NOT_FOUND, "Page not found.") })
        .with_state(WebState {
            client,
            available: Arc::new(Semaphore::new(1)),
        })
        .layer(DefaultBodyLimit::max(4096))
        .layer(middleware::from_fn(move |request, next| {
            local_request(request, next, port)
        }))
}

fn asset(content_type: &'static str, content: &'static str) -> Response {
    ([(header::CONTENT_TYPE, content_type)], content).into_response()
}

async fn local_request(request: Request, next: Next, port: u16) -> Response {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok());
    let valid_host = host.is_some_and(|host| {
        host == format!("127.0.0.1:{port}") || host == format!("localhost:{port}")
            // Browsers omit the default HTTP port from Host and Origin.
            || (port == 80 && matches!(host, "127.0.0.1" | "localhost"))
    });
    let valid_origin = request
        .headers()
        .get(header::ORIGIN)
        .is_none_or(|origin| host.is_some_and(|host| origin == format!("http://{host}").as_str()));
    let valid_action = request.method() != axum::http::Method::POST
        || request
            .headers()
            .get("x-skill-scanner")
            .is_some_and(|value| value == "1");
    let mut response = if valid_host && valid_origin && valid_action {
        next.run(request).await
    } else {
        error(
            StatusCode::FORBIDDEN,
            "Open the local Skill Scanner URL to use this interface.",
        )
    };
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn error(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({ "message": message }))).into_response()
}

async fn scan(
    State(state): State<WebState>,
    body: Result<Json<ScanRequest>, JsonRejection>,
) -> Response {
    let input = match body {
        Ok(Json(input)) => input,
        Err(rejection) => {
            return error(
                rejection.status(),
                "Send a JSON object with one repository field (maximum 4 KiB).",
            )
        }
    };
    let repository = match input.repository.trim().parse::<Repository>() {
        Ok(repository) => repository,
        Err(message) => return error(StatusCode::BAD_REQUEST, &message),
    };
    let Ok(permit) = state.available.try_acquire_owned() else {
        return error(
            StatusCode::CONFLICT,
            "A scan is already running. Wait for it to finish, then try again.",
        );
    };
    // A bounded channel applies backpressure without retaining a job/history.
    let (sender, receiver) = mpsc::channel::<Result<String, Infallible>>(8);
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let result = scanner::scan_with_progress(&state.client, &repository, |progress| {
            let (current, total) = match &progress {
                ScanProgress::Skill { current, total, .. } => (Some(*current), Some(*total)),
                _ => (None, None),
            };
            let mut message = Vec::new();
            report::write_progress(&mut message, progress)
                .expect("writing to a vector cannot fail");
            send(
                &sender,
                Event::Progress {
                    message: String::from_utf8(message)
                        .expect("progress is UTF-8")
                        .trim_end()
                        .to_owned(),
                    current,
                    total,
                },
            );
        });
        let event = match result {
            Ok(inventory) => Event::Complete {
                inventory: inventory.into(),
            },
            Err(error) => Event::Error {
                message: error.to_string(),
            },
        };
        send(&sender, event);
    });
    (
        [(header::CONTENT_TYPE, "application/x-ndjson; charset=utf-8")],
        Body::from_stream(ReceiverStream::new(receiver)),
    )
        .into_response()
}

fn send(sender: &mpsc::Sender<Result<String, Infallible>>, event: Event) {
    let mut line = serde_json::to_string(&event).expect("event contains serializable data");
    line.push('\n');
    // Disconnection discards the report. The scan finishes and releases its permit.
    let _ = sender.blocking_send(Ok(line));
}
