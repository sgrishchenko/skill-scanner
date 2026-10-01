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

## Organization scans

Keep owner listing, per-repository outcomes, and combined inventories in
`organization`. It calls the scanner's shared snapshot step with repository
details from the listing, so discovery, metadata, and caching stay identical to
single-repository scans; only the truncated-tree directory walk is disabled.
`ScanError` marks credential, rate-limit, retry-delay, and connection failures
as service failures, which stop an
[organization scan](features/organization-scan.md); other errors are recorded
for the affected repository. Repositories are scanned sequentially on the same
blocking client. The CLI and web server render the same
`OrganizationInventory`, and the web route reuses the scan worker, bounded
event channel, and semaphore.

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

## Starred skill storage

Keep starred skills in `starred`, separate from history and the cache, using
the shared `storage` helpers for state directories, flat repository filename
stems, bounded entry reads, removal, and atomic replacement. Recent
repositories use the same helpers. Each star is one versioned JSON file named
by its repository stem and a 64-bit FNV-1a hash of its path, which is stable
across platforms and releases. Listing verifies that each record's repository
and path produce its filename, so hash collisions or edited files are ignored
rather than misattributed. One file per star avoids read-modify-write races
between processes. CLI and web entry points configure `StarredSkills` from the
environment; web list, star, and unstar operations run on `spawn_blocking`.
The [starred skills feature](features/starred-skills.md) defines locations,
validation, and error behavior.

## Codex installation

Keep Codex directory selection, installation records, downloading, and
publishing in `codex`, separate from the state stores in `storage`. CLI and web
entry points configure `CodexSkills` from the environment. Installation reuses
the blocking `GitHubClient`, the scanner's tree validation and regular-file
rule, and the starred skill path and commit validation; it walks the pinned
tree to the skill directory and downloads blobs with a shared size budget.
Each installed folder carries its own versioned `.skill-scanner.json` record
instead of a separate index, so the directory itself is the source of truth
and folders created by people or other tools are recognized as unowned.
Files are staged in a hidden sibling folder and published with one rename;
replacement renames the old folder aside first and restores it on failure.
Web install and removal run on `spawn_blocking` behind their own one-permit
semaphore, so they neither wait for nor block scans. The
[Codex skills feature](features/codex-skills.md) defines locations, limits,
ownership, and error behavior.

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
| Organization scans | Local mock HTTP covers pagination, forks, private and duplicate entries, invalid listings, per-repository failures, service failures that stop the scan, cache reuse, reports, and the web stream |
| Analysis cache | Temporary directories and mocked HTTP verify persisted reuse, commit invalidation, cross-interface sharing, warnings, corrupt entries, failed refreshes, and unavailable storage |
| Recent repositories | Temporary storage and mock scans verify ordering, deduplication, successful-only recording, concurrent writes, corruption handling, cross-interface persistence, and removal; subprocess tests verify offline CLI use |
| Starred skills | Temporary storage verifies identity, validation, ordering, concurrent writes, corruption handling, and unavailable or disabled storage; in-process HTTP and subprocess tests verify the API, request protections, cross-interface persistence, and offline CLI use |
| Codex skills | Mocked HTTP and temporary directories verify copied files, executable modes, excluded entries, replacement, conflicts before network access, limits, unportable names, failed downloads, ownership records, and removal; in-process HTTP and subprocess tests verify the API, request protections, one change at a time, and offline CLI listing and removal |
| CLI | Subprocess tests verify exit codes and stdout/stderr separation |
| Web API and scan lifecycle | In-process HTTP tests exercise the real scanner against mocked GitHub responses, including streaming, warnings, errors, empty results, concurrency, disconnects, input/body limits, and Host/Origin checks |
| Embedded server | CLI subprocess tests start the site from another working directory and check port errors |
| Browser behavior | Acceptance checks use mocked scan responses for repeatability without a GitHub token; PR video demos record passing E2E tests with screenshot assertions at key steps, following the [video demo requirements](../CONTRIBUTING.md#visual-feature-demonstrations) |

The [browser E2E suite](../tests/e2e/README.md) runs the rebuilt executable in a
pinned Playwright Linux container, with fixed repository and organization scan
fixtures and isolated history, starred, Codex, and cache directories.
Recent-list/removal, starred-skill, and Codex list/removal/conflict requests and
CLI history, starred, and Codex reads use the actual application; only
GitHub-dependent installs are mocked. CI checks reviewed screenshot baselines without updates and
retains reports, videos, and failure artifacts. Demo exports use the video
attachment from that same passing test execution.
