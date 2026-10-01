use std::{io::Read, thread, time::Duration};

use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{
    blocking::Client,
    header::{self, HeaderMap, HeaderValue},
    redirect::Policy,
    StatusCode, Url,
};
use serde::{de::DeserializeOwned, Deserialize};

use crate::{
    repository::{Owner, Repository},
    ScanError,
};

pub const MAX_SKILL_BYTES: u64 = 1024 * 1024;
const MAX_API_BYTES: u64 = 32 * 1024 * 1024;
const ATTEMPTS: usize = 3;
pub(crate) const OWNER_PAGE_SIZE: usize = 100;

pub struct GitHubClient {
    client: Client,
    base: Url,
    retry_backoff: Duration,
    request_timeout: Duration,
    max_api_bytes: u64,
}

#[derive(Deserialize)]
pub(crate) struct RepositoryInfo {
    pub private: bool,
    pub default_branch: String,
}

/// One entry of an organization's or user's public repository listing.
#[derive(Deserialize)]
pub(crate) struct OwnerRepository {
    pub name: String,
    pub owner: Account,
    pub private: bool,
    pub fork: bool,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub default_branch: String,
}

#[derive(Deserialize)]
pub(crate) struct Account {
    pub login: String,
}

#[derive(Deserialize)]
pub(crate) struct Commit {
    pub sha: String,
    pub commit: CommitDetails,
}

#[derive(Deserialize)]
pub(crate) struct CommitDetails {
    pub tree: ObjectId,
}

#[derive(Deserialize)]
pub(crate) struct ObjectId {
    pub sha: String,
}

#[derive(Clone, Deserialize)]
pub(crate) struct Tree {
    pub sha: String,
    pub truncated: bool,
    pub tree: Vec<TreeEntry>,
}

#[derive(Clone, Deserialize)]
pub(crate) struct TreeEntry {
    pub path: String,
    pub mode: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub sha: String,
    pub size: Option<u64>,
}

#[derive(Deserialize)]
struct Blob {
    sha: String,
    size: u64,
    encoding: String,
    content: String,
}

pub(crate) enum BlobContent {
    Bytes(Vec<u8>),
    TooLarge,
}

struct ApiResponse {
    status: StatusCode,
    body: Vec<u8>,
}

impl GitHubClient {
    pub fn new(token: Option<&str>) -> Result<Self, ScanError> {
        Self::build(
            Url::parse("https://api.github.com/").expect("constant URL"),
            token,
            false,
            Duration::from_millis(250),
            Duration::from_secs(30),
            MAX_API_BYTES,
        )
    }

    fn build(
        base: Url,
        token: Option<&str>,
        no_proxy: bool,
        retry_backoff: Duration,
        timeout: Duration,
        max_api_bytes: u64,
    ) -> Result<Self, ScanError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        headers.insert(
            "X-GitHub-Api-Version",
            HeaderValue::from_static("2022-11-28"),
        );
        if let Some(token) = token {
            let mut auth = HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| {
                ScanError::new("GITHUB_TOKEN is invalid; unset it or supply a valid token.")
            })?;
            auth.set_sensitive(true);
            headers.insert(header::AUTHORIZATION, auth);
        }
        let origin = base.origin();
        let mut builder = Client::builder()
            .default_headers(headers)
            .user_agent(concat!("skill-scanner/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .timeout(timeout)
            .redirect(Policy::custom(move |attempt| {
                if attempt.previous().len() >= 5 || attempt.url().origin() != origin {
                    attempt.stop()
                } else {
                    attempt.follow()
                }
            }));
        if no_proxy {
            builder = builder.no_proxy();
        }
        let client = builder.build().map_err(|_| {
            ScanError::new(
                "Could not initialize the HTTPS client; check your system's network configuration.",
            )
        })?;
        Ok(Self {
            client,
            base,
            retry_backoff,
            request_timeout: timeout,
            max_api_bytes,
        })
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        base: Url,
        token: Option<&str>,
        timeout: Duration,
        max_api_bytes: u64,
    ) -> Self {
        Self::build(base, token, true, Duration::ZERO, timeout, max_api_bytes).unwrap()
    }

    fn url(&self, repository: &Repository, segments: &[&str]) -> Url {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .expect("API URL")
            .pop_if_empty()
            .extend(["repos", repository.owner.as_str(), repository.name.as_str()])
            .extend(segments.iter().copied());
        url
    }

    pub(crate) fn repository(&self, repository: &Repository) -> Result<RepositoryInfo, ScanError> {
        self.json(self.url(repository, &[]))
    }

    /// List one page of public repositories owned by an organization or user.
    pub(crate) fn owner_repositories(
        &self,
        owner: &Owner,
        page: usize,
    ) -> Result<Vec<OwnerRepository>, ScanError> {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .expect("API URL")
            .pop_if_empty()
            .extend(["users", owner.login.as_str(), "repos"]);
        url.query_pairs_mut()
            .append_pair("type", "owner")
            .append_pair("sort", "full_name")
            .append_pair("per_page", &OWNER_PAGE_SIZE.to_string())
            .append_pair("page", &page.to_string());
        let response = self.request(url, Some(StatusCode::NOT_FOUND))?;
        if response.status == StatusCode::NOT_FOUND {
            return Err(ScanError::new(
                "The organization or user is inaccessible on GitHub; check the name and retry. (HTTP 404)",
            ));
        }
        decode_success(response)
    }

    pub(crate) fn default_commit(
        &self,
        repository: &Repository,
        branch: &str,
    ) -> Result<Option<Commit>, ScanError> {
        let response = self.request(
            self.url(repository, &["commits", branch]),
            Some(StatusCode::CONFLICT),
        )?;
        if response.status == StatusCode::CONFLICT {
            // Only GitHub's specific empty-repository response is a successful
            // empty scan. Other 409 responses must not hide a failure.
            let is_empty = serde_json::from_slice::<serde_json::Value>(&response.body)
                .ok()
                .and_then(|value| {
                    value
                        .get("message")
                        .and_then(|message| message.as_str())
                        .map(|message| message.eq_ignore_ascii_case("Git Repository is empty."))
                })
                .unwrap_or(false);
            if is_empty {
                return Ok(None);
            }
        }
        let commit: Commit = decode_success(response)?;
        validate_sha(&commit.sha)?;
        validate_sha(&commit.commit.tree.sha)?;
        Ok(Some(commit))
    }

    pub(crate) fn tree(
        &self,
        repository: &Repository,
        sha: &str,
        recursive: bool,
    ) -> Result<Tree, ScanError> {
        let mut url = self.url(repository, &["git", "trees", sha]);
        if recursive {
            url.query_pairs_mut().append_pair("recursive", "1");
        }
        let tree: Tree = self.json(url)?;
        if tree.sha != sha {
            return Err(ScanError::new("GitHub returned a different tree object; the inventory is incomplete. Retry the scan."));
        }
        Ok(tree)
    }

    pub(crate) fn blob(
        &self,
        repository: &Repository,
        sha: &str,
    ) -> Result<BlobContent, ScanError> {
        self.blob_up_to(repository, sha, MAX_SKILL_BYTES)
    }

    /// Download a file of at most `max_bytes`; larger files are not decoded.
    pub(crate) fn blob_up_to(
        &self,
        repository: &Repository,
        sha: &str,
        max_bytes: u64,
    ) -> Result<BlobContent, ScanError> {
        let blob: Blob = self.json(self.url(repository, &["git", "blobs", sha]))?;
        if blob.sha != sha {
            return Err(ScanError::new("GitHub returned a different file object; the inventory is incomplete. Retry the scan."));
        }
        if blob.size > max_bytes {
            return Ok(BlobContent::TooLarge);
        }
        if blob.encoding != "base64" {
            return Err(ScanError::new("GitHub returned an unsupported file encoding; the inventory is incomplete. Retry the scan."));
        }
        let compact: Vec<u8> = blob
            .content
            .bytes()
            .filter(|byte| !byte.is_ascii_whitespace())
            .collect();
        // Limit allocation even if the API's declared file size is inconsistent.
        if compact.len() as u64 > max_bytes.div_ceil(3) * 4 {
            return Err(ScanError::new("GitHub returned inconsistent file size data; the inventory is incomplete. Retry the scan."));
        }
        let bytes = STANDARD.decode(compact).map_err(|_| {
            ScanError::new(
                "GitHub returned invalid file data; the inventory is incomplete. Retry the scan.",
            )
        })?;
        if bytes.len() as u64 != blob.size {
            return Err(ScanError::new(
                "GitHub returned an incomplete file; retry the scan.",
            ));
        }
        Ok(BlobContent::Bytes(bytes))
    }

    fn json<T: DeserializeOwned>(&self, url: Url) -> Result<T, ScanError> {
        decode_success(self.request(url, None)?)
    }

    /// Return error responses with the `inspect` status for the caller to interpret.
    fn request(&self, url: Url, inspect: Option<StatusCode>) -> Result<ApiResponse, ScanError> {
        for attempt in 0..ATTEMPTS {
            // A per-request timeout also applies to the underlying async body.
            // The blocking client's timeout alone restarts for each read.
            let response = self
                .client
                .get(url.clone())
                .timeout(self.request_timeout)
                .send();
            let response = match response {
                Ok(response) => response,
                Err(error) => {
                    if (error.is_connect() || error.is_timeout() || error.is_body())
                        && attempt + 1 < ATTEMPTS
                    {
                        thread::sleep(self.retry_backoff * (1 << attempt));
                        continue;
                    }
                    return Err(ScanError::service("Could not reach GitHub or the request timed out; check your connection and retry the scan."));
                }
            };
            let status = response.status();
            if matches!(status.as_u16(), 408 | 429 | 500 | 502 | 503 | 504)
                && attempt + 1 < ATTEMPTS
            {
                let delay = if let Some(retry_after) = response.headers().get(header::RETRY_AFTER) {
                    match retry_after
                        .to_str()
                        .ok()
                        .and_then(|value| value.parse::<u64>().ok())
                    {
                        Some(seconds) if seconds <= 5 => Duration::from_secs(seconds),
                        _ => return Err(ScanError::service(
                            "GitHub asked for a longer retry delay; wait and retry the scan later.",
                        )),
                    }
                } else {
                    self.retry_backoff * (1 << attempt)
                };
                drop(response);
                thread::sleep(delay);
                continue;
            }
            // Do not parse or print arbitrary server error text. A 409 body is
            // needed solely to distinguish an empty repository from failure.
            if !status.is_success() && Some(status) != inspect {
                return Err(http_error(status));
            }
            if response
                .content_length()
                .is_some_and(|length| length > self.max_api_bytes)
            {
                return Err(response_too_large());
            }
            let mut body = Vec::new();
            if response
                .take(self.max_api_bytes + 1)
                .read_to_end(&mut body)
                .is_err()
            {
                if attempt + 1 < ATTEMPTS {
                    thread::sleep(self.retry_backoff * (1 << attempt));
                    continue;
                }
                return Err(ScanError::service("GitHub's response was interrupted or timed out; the inventory is incomplete. Check your connection and retry the scan."));
            }
            if body.len() as u64 > self.max_api_bytes {
                return Err(response_too_large());
            }
            return Ok(ApiResponse { status, body });
        }
        unreachable!("each final attempt returns a response or error")
    }
}

pub(crate) fn validate_sha(sha: &str) -> Result<(), ScanError> {
    if !matches!(sha.len(), 40 | 64) || !sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ScanError::new(
            "GitHub returned an invalid object ID; the inventory is incomplete. Retry the scan.",
        ));
    }
    Ok(())
}

fn decode_success<T: DeserializeOwned>(response: ApiResponse) -> Result<T, ScanError> {
    if !response.status.is_success() {
        return Err(http_error(response.status));
    }
    serde_json::from_slice(&response.body)
        .map_err(|_| ScanError::new("GitHub returned invalid or incomplete JSON; retry the scan."))
}

fn response_too_large() -> ScanError {
    ScanError::new("GitHub's API response exceeds the 32 MiB limit; the inventory is incomplete. Try a smaller repository.")
}

fn http_error(status: StatusCode) -> ScanError {
    let message = match status.as_u16() {
        401 => "GitHub rejected GITHUB_TOKEN; unset it for anonymous access or supply a valid token.",
        403 | 429 => "GitHub denied access or its API rate limit was reached; wait and retry, or set a valid GITHUB_TOKEN for higher public-repository limits.",
        404 => "The repository or an object is inaccessible on GitHub; check the repository name and that it is public, then retry.",
        408 | 500 | 502 | 503 | 504 => "GitHub is temporarily unavailable; retry the scan later.",
        300..=399 => "GitHub redirected outside the API origin or too many times; retry with the repository's current GitHub URL.",
        _ => "GitHub could not complete the request; verify the public repository URL and retry the scan.",
    };
    let message = format!("{message} (HTTP {})", status.as_u16());
    if matches!(status.as_u16(), 401 | 403 | 429) {
        ScanError::service(message)
    } else {
        ScanError::new(message)
    }
}
