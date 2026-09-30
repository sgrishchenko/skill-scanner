# First-version implementation decisions

This implements the proposed first version in [cli.md](cli.md) as a Rust Cargo
binary named `skill-scanner`. The choices below define the open technical details
before implementation.

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
  succeeds. Warnings and errors go to stderr. Escape control characters from
  repository content, including terminal escape sequences. GitHub links are
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
