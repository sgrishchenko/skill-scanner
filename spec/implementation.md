# Implementation decisions

Status: implemented. This document records the architecture supporting the
[feature specifications](README.md). Behavior, limits, and acceptance checks
live with the feature that owns them.

## Runtime and modules

Use a Rust Cargo binary crate with Rust 1.84 or newer and edition 2021. Commit
`Cargo.lock` and use `--locked` for builds and installation. Keep command-line
argument parsing, repository input, GitHub access, skill discovery, metadata
parsing, aggregation, and report rendering separate so they can be tested with
repository fixtures and mocked HTTP responses.

The [installation and release feature](features/installation-and-releases.md)
defines supported platforms and packaging.

## GitHub client and scanner

Use the GitHub REST API over HTTPS with the 2022-11-28 API version and an
application User-Agent. The blocking client implements the timeouts, retries,
redirect restrictions, credentials, and limits in
[repository access](features/repository-access.md).

Resolve the default branch once, then scan tree/blob object IDs as defined by
[skill discovery](features/skill-discovery.md). Parse YAML front matter under
the [metadata rules](features/skill-metadata.md). Both interfaces use the same
scanner and progress events.

## Persistent analysis cache

Keep disk serialization, versioning, location selection, and atomic replacement
in `cache`, separate from GitHub transport and discovery. CLI and web entry
points configure the same `ScanCache` from the environment and invoke the shared
`scan_with_cache` pipeline. Uncached scanner helpers and explicit cache paths
support isolated tests without touching a user's cache.

The scanner checks the [analysis cache](features/analysis-cache.md) only after
live repository access and default-branch commit resolution, then stores only
completed inventories. JSON entries contain the repository, full commit, skill
metadata and warnings, a format version, and the scanner package version. Bump
the format version when discovery or metadata semantics change without a
package version bump. Recompute aggregation from the cached inventory using
shared Rust logic.

Use the shared `storage` helper for atomic file replacement.
Use one case-insensitive repository path per entry, with prefixed components
safe for supported filesystems. Limit cache reads and stored entries to 32 MiB.
Write a unique sibling temporary file, sync and close it, then rename it over
the entry. Failed writes leave the prior entry intact and remove the temporary
file when possible. No database, lock service, or cache dependency is needed.

## Recent repository storage

Keep lightweight repository summaries in `recent`, separate from the disposable
analysis cache, using the shared atomic-file writer in `storage`. CLI and web
entry points configure `RecentRepositories` from the environment and call
`scan_with_storage`, which wraps the existing cached scanner. Only a successful
inventory updates history. A history write failure emits a warning without
changing scan success; the web protocol carries it as `history_warning`.

Each repository has one flat, case-insensitive, validated filename and a
versioned JSON record. Per-repository atomic replacement avoids a shared index
and preserves concurrent writes to different repositories. Listing reads disk
and sorts completion timestamps. History does not depend on the scanner package
version, cached inventories, browser storage, or server port. Web list/removal
operations run on `spawn_blocking` and use the normal local request protections.
The [recent repositories feature](features/recent-repositories.md) defines
locations, retention, error behavior, and concurrency semantics.

## Shared aggregation

Implement [similarity grouping](features/similarity-grouping.md) in shared Rust
logic. Hash-map lookups and iterative disjoint-set traversal merge connected
matches without comparing every pair. Members reference original inventory
indices to avoid duplicating skill data. The CLI renders and the web server
serializes the same aggregation result.

## Embedded web stack

Use Axum 0.8 on Tokio to serve embedded HTML, CSS, JavaScript, and an SVG
favicon. Run the existing blocking GitHub client on `spawn_blocking`; construct
and drop the client outside the async runtime.

A bounded channel of eight events provides backpressure for the
[NDJSON scan protocol](features/web-scan-api.md). A semaphore permits one scan
per server. Disconnections drop the response while the worker retains capacity
until its current scan finishes, following the
[web scan lifecycle](features/web-scan.md#concurrency-and-state).

The [local web server](features/local-web-server.md) owns delivery, request
protection, and browser-content rules. The browser keeps the current inventory
in page memory and performs [results filtering](features/web-results.md#search-and-filters)
locally.

## Verification

Tests use local fixtures and mocked HTTP responses, never live GitHub access or
real credentials.

| Area | Verification approach |
| --- | --- |
| Repository input, metadata, aggregation, and terminal rendering | Unit tests for parsing, grouping, ordering, warnings, output, and escaping |
| GitHub access and discovery | Fixtures and local mock HTTP cover snapshots, authentication, truncation fallback, limits, retries, failures, and empty repositories |
| Analysis cache | Temporary directories and mocked HTTP verify persisted reuse, commit invalidation, cross-interface sharing, warnings, corrupt entries, failed refreshes, and unavailable storage |
| Recent repositories | Temporary storage and mock scans verify ordering, deduplication, successful-only recording, concurrent writes, corruption handling, cross-interface persistence, and removal; subprocess tests verify offline CLI use |
| CLI | Subprocess tests verify exit codes and stdout/stderr separation |
| Web API and scan lifecycle | In-process HTTP tests exercise the real scanner against mocked GitHub responses, including streaming, warnings, errors, empty results, concurrency, disconnects, input/body limits, and Host/Origin checks |
| Embedded server | CLI subprocess tests start the site from another working directory and check port errors |
| Browser behavior | Acceptance checks use mocked scan responses for repeatability without a GitHub token |
