# Repository access

Status: implemented. Shared by the CLI and local web interface.
See the [specification index](../README.md) for product scope.

## Repository input

Accept one `owner/repo` identifier or HTTPS `github.com` repository URL,
optionally ending in `/` or `.git`. Reject credentials, query strings,
fragments, ports, local paths, and branch/file URLs. Branch, tag, and commit
selection are deferred.

[Organization scans](organization-scan.md#input-and-listing) instead accept one
`OWNER` or `https://github.com/OWNER` and list that owner's public repositories.

The [web form](web-scan.md) trims surrounding whitespace before applying the
same repository or owner validation. Invalid CLI input is a usage error; the
[CLI report](cli-report.md#exit-codes) and
[web scan API](web-scan-api.md#request-validation) define interface-specific
error handling.

## GitHub access

Use the GitHub REST API over HTTPS without requiring a local Git installation.
Public repositories can be scanned without credentials, subject to GitHub's
anonymous API limits. An optional process-level `GITHUB_TOKEN` raises public
repository API limits. Reject private repositories even when the token can
access them.

Even with a cached analysis, every scan checks repository access and resolves
the current default-branch commit online. Organization scans take repository
details from the owner's public repository listing. Cache hits skip only tree/blob reads;
failed validation cannot return stale results. See the
[analysis cache](analysis-cache.md#freshness-and-reuse).

Never include credentials or server error bodies in reports or diagnostics.
The browser never asks for, serializes, persists, or receives the token. The
web server exposes no GitHub API base-URL override.

## Timeouts, retries, and limits

- Use a 10-second connection timeout and a 30-second total timeout per request,
  including reading its body.
- Allow redirects only within the API origin, with at most five hops.
- Make at most three attempts for connection/timeouts, interrupted responses,
  HTTP 408/429, and HTTP 500/502/503/504. Backoff is 250 ms then 500 ms.
- Honor integer `Retry-After` values up to five seconds. Longer or unsupported
  values fail with a retry-later diagnostic.
- Fail immediately on HTTP 403 rate limits with guidance. Do not retry other
  HTTP failures. Credential, rate-limit, retry-delay, and connection failures
  stop an [organization scan](organization-scan.md#repository-scans); other
  failures affect only the repository being scanned.
- Read at most 32 MiB per API JSON response. An oversized or malformed API
  listing, HTTP failure, or failed blob download fails the entire scan.
  [Skill metadata](skill-metadata.md) defines the separate 1 MiB file limit.

Inaccessible repositories, network failures, rate limits, and incomplete scans
produce a clear diagnostic with a suggested next step where possible. These
failures must not be presented as successful empty or partial inventories; see
[scan outcomes](skill-discovery.md#scan-outcomes).

## Acceptance checks

- A public repository can be scanned without configuring credentials.
- Identifiers and repository URLs with the supported suffixes are accepted;
  unsupported inputs are rejected by both interfaces.
- Supplying a token does not enable private repository scans or expose the
  token in diagnostics or browser responses.
- Transient failures respect the attempt limit and backoff; long retry delays
  and HTTP 403 rate limits produce actionable failures.
- An inaccessible repository, oversized API listing, or failed download fails
  clearly instead of reporting zero results.
