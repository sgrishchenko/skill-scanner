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
use tokio::sync::{mpsc, OwnedSemaphorePermit, Semaphore};
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    aggregation::{self, Aggregation},
    cache::ScanCache,
    github::GitHubClient,
    organization::{self, OrganizationInventory, OrganizationProgress, RepositorySummary},
    recent::RecentRepositories,
    report,
    repository::{Owner, Repository},
    scanner::{self, Inventory, ScanProgress, Skill},
};

const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

#[derive(Clone)]
struct WebState {
    client: Arc<GitHubClient>,
    cache: ScanCache,
    recent: RecentRepositories,
    available: Arc<Semaphore>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScanRequest {
    repository: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OrganizationRequest {
    organization: String,
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
struct WebOrganization {
    organization: String,
    repositories: Vec<RepositorySummary>,
    skipped_forks: usize,
    skills: Vec<Skill>,
    aggregation: Aggregation,
}

impl From<OrganizationInventory> for WebOrganization {
    fn from(inventory: OrganizationInventory) -> Self {
        Self {
            aggregation: aggregation::aggregate(&inventory.skills),
            organization: inventory.owner.to_string(),
            repositories: inventory.repositories,
            skipped_forks: inventory.skipped_forks,
            skills: inventory.skills,
        }
    }
}

/// Clients know which shape to expect from the route they called.
#[derive(Serialize)]
#[serde(untagged)]
enum CompletedInventory {
    Repository(WebInventory),
    Organization(WebOrganization),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Event {
    Progress {
        message: String,
        current: Option<usize>,
        total: Option<usize>,
    },
    HistoryWarning {
        message: String,
    },
    Complete {
        inventory: CompletedInventory,
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
        axum::serve(
            listener,
            router(
                client.clone(),
                port,
                ScanCache::from_environment(),
                RecentRepositories::from_environment(),
            ),
        )
        .await
    })
}

pub(crate) fn router(
    client: Arc<GitHubClient>,
    port: u16,
    cache: ScanCache,
    recent: RecentRepositories,
) -> Router {
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
        .route("/api/scan-org", post(scan_organization))
        .route("/api/recent", get(list_recent))
        .route("/api/recent/remove", post(remove_recent))
        .fallback(|| async { error(StatusCode::NOT_FOUND, "Page not found.") })
        .with_state(WebState {
            client,
            cache,
            recent,
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

async fn list_recent(State(state): State<WebState>) -> Response {
    let enabled = state.recent.is_enabled();
    match tokio::task::spawn_blocking(move || state.recent.list()).await {
        Ok(Ok(repositories)) => Json(serde_json::json!({
            "enabled": enabled,
            "repositories": repositories,
        }))
        .into_response(),
        _ => history_error(),
    }
}

async fn remove_recent(
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
    match tokio::task::spawn_blocking(move || state.recent.remove(&repository)).await {
        Ok(Ok(())) => StatusCode::NO_CONTENT.into_response(),
        _ => history_error(),
    }
}

fn history_error() -> Response {
    error(StatusCode::INTERNAL_SERVER_ERROR, "Could not access recent repositories. Check SKILL_SCANNER_HISTORY_DIR and directory permissions.")
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
        return busy();
    };
    stream_events(permit, move |emit| {
        let result = scanner::scan_with_storage(
            &state.client,
            &repository,
            &state.cache,
            &state.recent,
            |progress| {
                if let ScanProgress::HistoryWarning(message) = &progress {
                    emit(Event::HistoryWarning {
                        message: (*message).to_owned(),
                    });
                    return;
                }
                let (current, total) = match &progress {
                    ScanProgress::Skill { current, total, .. } => (Some(*current), Some(*total)),
                    _ => (None, None),
                };
                emit(Event::Progress {
                    message: progress_message(|output| report::write_progress(output, progress)),
                    current,
                    total,
                });
            },
        );
        match result {
            Ok(inventory) => Event::Complete {
                inventory: CompletedInventory::Repository(inventory.into()),
            },
            Err(error) => Event::Error {
                message: error.to_string(),
            },
        }
    })
}

async fn scan_organization(
    State(state): State<WebState>,
    body: Result<Json<OrganizationRequest>, JsonRejection>,
) -> Response {
    let input = match body {
        Ok(Json(input)) => input,
        Err(rejection) => {
            return error(
                rejection.status(),
                "Send a JSON object with one organization field (maximum 4 KiB).",
            )
        }
    };
    let owner = match input.organization.trim().parse::<Owner>() {
        Ok(owner) => owner,
        Err(message) => return error(StatusCode::BAD_REQUEST, &message),
    };
    let Ok(permit) = state.available.try_acquire_owned() else {
        return busy();
    };
    stream_events(permit, move |emit| {
        // Repository counters drive the progress bar; nested skill counters
        // remain in the message text.
        let mut current: Option<(String, usize, usize)> = None;
        let result =
            organization::scan_organization(&state.client, &owner, &state.cache, |progress| {
                let message = match progress {
                    OrganizationProgress::Scan(progress) => {
                        let (repository, _, _) =
                            current.as_ref().expect("repository progress comes first");
                        format!(
                            "{repository}: {}",
                            progress_message(|output| report::write_progress(output, progress))
                        )
                    }
                    OrganizationProgress::Repository {
                        repository,
                        current: index,
                        total,
                    } => {
                        current = Some((repository.to_string(), index, total));
                        progress_message(|output| {
                            report::write_organization_progress(output, progress)
                        })
                    }
                    OrganizationProgress::Listing { .. } => progress_message(|output| {
                        report::write_organization_progress(output, progress)
                    }),
                };
                emit(Event::Progress {
                    message,
                    current: current.as_ref().map(|(_, index, _)| *index),
                    total: current.as_ref().map(|(_, _, total)| *total),
                });
            });
        match result {
            Ok(inventory) => Event::Complete {
                inventory: CompletedInventory::Organization(inventory.into()),
            },
            Err(error) => Event::Error {
                message: error.to_string(),
            },
        }
    })
}

fn busy() -> Response {
    error(
        StatusCode::CONFLICT,
        "A scan is already running. Wait for it to finish, then try again.",
    )
}

fn progress_message(write: impl FnOnce(&mut Vec<u8>) -> io::Result<()>) -> String {
    let mut message = Vec::new();
    write(&mut message).expect("writing to a vector cannot fail");
    String::from_utf8(message)
        .expect("progress is UTF-8")
        .trim_end()
        .to_owned()
}

/// Run one scan on a blocking worker and stream its events. The worker keeps
/// the permit until the scan finishes, even if the client disconnects.
fn stream_events(
    permit: OwnedSemaphorePermit,
    work: impl FnOnce(&mut dyn FnMut(Event)) -> Event + Send + 'static,
) -> Response {
    // A bounded channel applies backpressure without retaining a scan job.
    let (sender, receiver) = mpsc::channel::<Result<String, Infallible>>(8);
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let event = work(&mut |event| send(&sender, event));
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
