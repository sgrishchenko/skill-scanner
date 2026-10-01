# Analysis cache

Status: implemented. The CLI and local web interface share a persistent cache
of complete public-repository analyses through the
[scanner](skill-discovery.md).

## Freshness and reuse

1. On every scan, fetch repository details, require a public repository, and
   resolve its current default branch to a commit under the existing
   [repository access](repository-access.md) rules.
   [Organization scans](organization-scan.md) take the details from the owner's
   current repository listing and share entries with repository scans. Cached results never bypass
   these checks, including after a default-branch change.
2. Look up the repository by case-insensitive owner and name. Supported URL,
   identifier, `.git`, and trailing-slash forms share an entry. Different
   repositories have separate entries even if their commit IDs match.
3. Reuse an entry only when its full commit SHA, cache format version, and
   scanner package version match. Age is not a validity check; each request
   must resolve the current commit online. Access, network, rate-limit, or
   commit-resolution failures remain failures, with no stale-cache fallback.
4. On a hit, return the complete inventory without tree/blob requests or YAML
   parsing. Preserve sorted paths, metadata, and warnings. Regenerate pinned
   links and root fallback names using the current input's repository spelling.
   Shared [aggregation](similarity-grouping.md) runs on this inventory as usual.
5. On a miss or changed commit, scan the newly resolved snapshot normally.
   Store the result only after successful completion, including scans with
   metadata warnings or zero skills. A failed or partial scan never replaces a
   completed entry. Empty repositories have no commit and are not cached.

## Storage and controls

Enable caching by default for both commands, shared across invocations, browser
tabs, and server restarts under the same user and cache configuration. Keep one
complete entry per repository; a successful new analysis replaces the previous
one. This is an optimization, separate from the
[recent repository list](recent-repositories.md), and not an offline browsing API.

| Platform | Default directory |
| --- | --- |
| Linux and other Unix systems | `$XDG_CACHE_HOME/skill-scanner/scans` when XDG_CACHE_HOME is absolute and nonempty; otherwise `$HOME/.cache/skill-scanner/scans` |
| macOS | `$HOME/Library/Caches/skill-scanner/scans` |
| Windows | `%LOCALAPPDATA%\skill-scanner\scans` |

`SKILL_SCANNER_CACHE_DIR` overrides the whole directory for either command.
Relative overrides resolve against the process's working directory. An empty
value disables caching. If no platform directory can be resolved, run without
the cache. Create directories only when saving a complete analysis. Deleting
the cache directory clears cached analyses; the next scan rebuilds its entry.

Persist only the inventory and version identifiers, never credentials, raw
skill files, or GitHub error bodies. Limit each serialized entry to 32 MiB;
larger inventories still succeed but are not cached. Validate cached structure,
repository, commit, paths, ordering, and warning fields before reuse. Missing,
unreadable, malformed, incompatible, and oversized entries are cache misses.

Cache read/write failures do not fail an otherwise successful scan. Publish a
completed entry by replacing its file atomically, so concurrent CLI/server
processes and interrupted writes cannot expose partial JSON as a valid entry.
Concurrent writers may replace each other's entries; commit validation still
prevents reuse for a different current commit.

## Progress and web lifecycle

Both interfaces first report repository and default-branch resolution. A hit
then reports `Using cached analysis for commit: <full-sha>` and completes,
without fabricated directory or per-skill download progress.

The [web API](web-scan-api.md) sends this as a normal progress event with null
`current` and `total`, followed by the usual full completion inventory. No new
request fields or inventory fields are required. The browser remains
indeterminate until completion and renders cached results with the same
warnings, groups, links, and filters as fresh results.

The [one-scan-per-server rule](web-scan.md#concurrency-and-state) also covers
cache validation and reuse. A disconnected scan that finishes successfully can
populate the cache. Reloading the browser clears displayed results; submitting
the repository again revalidates the commit before using disk data.

## Acceptance checks

Use temporary cache directories, local repository fixtures, and mocked GitHub
or browser scan responses, without real credentials or live GitHub access.

- A first scan fetches and analyzes files. A second scan through a new cache
  instance makes only repository and current-commit requests and returns the
  same inventory, warnings, terminal report, and aggregation.
- CLI and web scans can reuse each other's entries across cache instances.
  URL/identifier and case variants share an entry; distinct repositories do not.
- A changed default-branch commit triggers fresh discovery and metadata reads,
  updates pinned links, and replaces the previous entry after success.
- Cached data cannot bypass private/inaccessible repository checks, failed
  commit resolution, or a failed refresh. Partial results are never cached.
- Corrupt, incompatible, mismatched, and oversized entries trigger a fresh
  scan. An unavailable directory or disabled cache still allows scans to finish.
- Zero-skill inventories with a commit are reusable. Empty repositories are
  rechecked and return no commit without substituting an older cached result.
- Browser cache-hit progress arrives before the completion event, without
  skill counters or partial results. Warnings, filtering, retry, and clearing
  results on reload behave as for fresh scans.
- A disconnected successful web scan populates the cache and releases its
  concurrency permit only after finishing.
