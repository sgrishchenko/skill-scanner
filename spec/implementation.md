# Implementation decisions

Status: implemented. These decisions describe the Rust Cargo binary
`skill-scanner`, its [CLI](cli.md), and its [local web interface](web.md).

- **Toolchain:** Rust 1.84 or newer, edition 2021; commit `Cargo.lock` and use
  `--locked` for builds and installation.
- **Platforms:** Linux (glibc), macOS, and Windows, on x86-64 and ARM64.
  Native CI jobs build and test each of these six targets. Linux release binaries
  require glibc 2.39 or newer; Cargo source builds can use the host's older glibc.
- **Transport:** GitHub REST API over HTTPS with a 10-second connection timeout
  and a 30-second total timeout per request, including reading its body. Redirects
  are allowed only within the API origin, with at most five hops. API requests
  use the 2022-11-28 API version and an application User-Agent. No Git installation
  is needed. `GITHUB_TOKEN` is optional; private repositories are rejected even
  when a supplied token can access them.
- **Retries:** at most three attempts for connection/timeouts, interrupted
  responses, HTTP 408/429, and HTTP 500/502/503/504. Backoff is 250 ms then 500 ms.
  Honor integer `Retry-After` values up to five seconds; longer or unsupported
  values fail with a retry-later diagnostic. HTTP 403 rate limits fail immediately
  with guidance. Other HTTP failures are not retried. Tokens and server error
  bodies are never echoed in diagnostics.
- **Limits:** read at most 1 MiB per `SKILL.md` and 32 MiB per API JSON response.
  Oversized, non-UTF-8, or invalid-front-matter skill files remain in the inventory
  with metadata warnings. An oversized or malformed API listing, HTTP failure,
  or failed blob download makes the entire scan fail with exit code 1.
- **Snapshot and discovery:** resolve the default branch once to its commit and
  root tree, then use only tree/blob object IDs. Try a recursive Git tree request;
  if truncated, discard it and walk nonrecursive trees. Fail if even a
  nonrecursive listing is truncated. Regular modes `100644` and `100755` qualify;
  symlinks and submodules are skipped. Tree paths, including hidden directories,
  are sorted before rendering. Empty repositories have no commit.
- **Metadata:** parse UTF-8 YAML front matter delimited by `---` lines (a `...`
  closing delimiter is also accepted), allowing a UTF-8 BOM and CRLF. Nonempty
  string `name` and `description` fields are required for metadata; missing,
  empty, malformed, or wrongly typed fields produce path/field warnings. Other
  fields are ignored. Directory/repository fallback names preserve discovery.
- **Terminal:** render a complete inventory to stdout only after the scan
  succeeds. Progress, warnings, and errors go to stderr. Flush plain-text progress
  before resolving the repository/branch, discovering files, walking each
  directory in a truncated-tree fallback, and scanning each skill. Skill progress
  includes the current path and a one-based counter out of the discovered total.
  Escape control characters from repository content, including terminal escape
  sequences. GitHub links are
  URL encoded and pinned to the scanned commit. Redirected output works without
  colors, a pager, or a TTY. Broken stdout pipes exit successfully.
- **Input:** accept `owner/repo` or an HTTPS `github.com` repository URL, optionally
  ending in `/` or `.git`; reject credentials, query strings, fragments, ports,
  local paths, and branch/file URLs. No branch selector or JSON mode is added.
- **Installation/release:** `cargo install --path . --locked` builds from source.
  A `v*` tag triggers the release workflow, which tests and builds all six native
  targets, packages the standalone executable with its license and README, and
  publishes a draft GitHub release with SHA-256 checksums. Users extract the
  archive and put the executable on `PATH`. Linux/macOS use `.tar.gz`; Windows
  uses `.zip`. Rust is required only for source builds.
- **Verification:** unit tests cover input, metadata, and rendering; fixture and
  local mock-HTTP tests cover discovery, snapshot consistency, authentication,
  truncation fallback, limits, retries, failures, and empty repositories.
  CLI subprocess tests verify exit codes and stdout/stderr separation. Tests
  never require live GitHub access or real credentials.

## Web interface decisions

- **Delivery:** `skill-scanner serve [--port PORT]` serves embedded HTML, CSS,
  JavaScript, and an SVG favicon from the existing binary. Default port: 3000;
  port 0 selects an available port. Print the actual URL to stderr. Users open
  that URL and stop the server with Ctrl+C. No browser auto-launch, asset folder,
  Node.js runtime, frontend build, CDN, or external font service is required.
- **HTTP stack:** Axum 0.8 on Tokio, with the existing blocking GitHub client
  running on `spawn_blocking`. The client is constructed and dropped outside
  the async runtime. This keeps network/discovery behavior shared with the CLI.
- **Transport:** one `POST /api/scan` per scan, with a JSON repository input and
  newline-delimited JSON progress, completion, or error events. A bounded channel
  of eight events provides backpressure. There are no server job IDs, polling
  endpoints, saved scans, or database. Only the final completion event contains
  an inventory. The protocol and statuses are recorded in [web.md](web.md).
- **Concurrency:** a semaphore permits one scan per server, across all tabs.
  Additional scans receive HTTP 409. Disconnecting drops the response; the
  worker completes its current scan before freeing capacity. No cancel control
  is offered because disconnecting does not cancel GitHub work.
- **Local access:** bind IPv4 loopback only; no configurable public bind address.
  Validate Host against `127.0.0.1:PORT` or `localhost:PORT` (the port can be
  omitted for HTTP port 80), and any Origin
  against the exact HTTP origin for that Host. POST requires the custom
  `X-Skill-Scanner: 1` header and JSON. Request bodies are limited to 4 KiB.
  No CORS access is granted. This prevents other websites from using the local
  process's GitHub credentials through browser requests or DNS rebinding.
- **Browser content:** serve a restrictive Content Security Policy; scripts,
  styles, images, and fetches are same-origin only, with framing disabled.
  Add `nosniff`, `no-referrer`, and `no-store`. Render all repository text with
  DOM text nodes, without HTML/Markdown evaluation. Source links must use the
  HTTPS GitHub origin and open with `noopener noreferrer`.
- **Credentials:** reuse the optional process-level `GITHUB_TOKEN`; never ask
  for, serialize, persist, or return it in the browser. Private repositories
  remain unsupported. No GitHub API base-URL override is exposed by the server.
- **Aggregation:** shared Rust logic groups valid matching names or descriptions
  after lowercase and punctuation/whitespace normalization. Hash-map lookups and
  iterative disjoint-set traversal merge connected matches without comparing
  every pair. Fallback names and unavailable/empty fields never create matches.
  Each skill belongs to one group. Counts cover similar groups (at least two
  members), grouped and standalone skills, largest similar group, and skills
  with warnings. CLI and web serialize/render the same aggregation result;
  members reference original inventory indices to avoid duplicating skill data.
- **UI:** responsive, keyboard-accessible form, streaming progress, repository
  and commit summary, grouped and path-sorted views, case-insensitive search
  across names/descriptions/paths, and similarity/metadata-warning filters.
  Groups show member counts, percentages, warnings, and expandable cards.
  Summary and group statistics always describe the full scan; filtering changes
  only visible members. Search/filtering makes no additional GitHub requests.
- **State:** keep only the current inventory in page memory. Clear previous
  results when a new scan starts; failures cannot leave a stale inventory
  labeled complete. Reloading clears the page. Distinguish no matching filters,
  a completed scan with no skills, an empty repository, and a failed scan.
- **Verification:** in-process HTTP tests exercise real scanner code against
  mocked GitHub responses, including streaming events, warnings, errors, empty
  results, concurrency, and disconnects. Tests cover input/body limits and
  origin/Host checks. CLI subprocess tests start the embedded site from another
  working directory and check port errors. Browser acceptance checks use mocked
  scan responses for repeatability and do not require a GitHub token.
