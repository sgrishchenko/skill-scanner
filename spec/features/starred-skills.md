# Starred skills

Status: implemented. People star skills from [web results](web-results.md) to
keep them in a persistent list shared by the web interface and CLI. Stars are
local bookmarks: they are not GitHub stars, are never sent to GitHub, and are
not quality or safety ratings.

## Starring and persistence

- Identify a star by its case-insensitive owner/repository and exact,
  case-sensitive repository-relative `SKILL.md` path. All repository input
  forms share that star. Starring an already starred skill replaces it.
- Store only the normalized repository, path, display name (at most 200
  characters; longer names are shortened), the full scanned commit, the star
  time (UTC milliseconds since the Unix epoch), and a storage format version.
  Do not save descriptions, credentials, raw content, or scan results.
- The star keeps the commit and name from when it was starred, and its source
  link is pinned to that commit. A later scan of the same repository shows the
  star on any skill at the same path, whatever its commit. Unstar and star
  again to record the newer commit and name.
- Accept only paths a scan can report: `SKILL.md` or a path ending in
  `/SKILL.md`, without empty, `.`, or `..` segments, and at most 1024 bytes.
  Commits are 40 or 64 lowercase hexadecimal characters, and names are nonempty.
- Share stars between CLI invocations, tabs, server restarts, and ports under
  the same user and storage configuration. Read disk on each list request.
  Stars are independent of [recent repositories](recent-repositories.md) and
  the [analysis cache](analysis-cache.md); removing a recent entry or deleting
  or disabling the cache keeps them.

| Platform | Default starred skills directory |
| --- | --- |
| Linux and other Unix systems | `$XDG_STATE_HOME/skill-scanner/starred` when absolute and nonempty; otherwise `$HOME/.local/state/skill-scanner/starred` |
| macOS | `$HOME/Library/Application Support/skill-scanner/starred` |
| Windows | `%LOCALAPPDATA%\skill-scanner\starred` |

`SKILL_SCANNER_STARRED_DIR` overrides the entire directory. Relative values
resolve against the process working directory; an empty value disables
starring. If no platform directory can be resolved, starring is disabled.
Create the directory only when starring a skill. Stars remain until unstarred.

Publish each star as its own versioned JSON file, replaced atomically and named
by its repository and a stable hash of its path. Concurrent stars of different
skills preserve each other; concurrent operations on the same skill use the
last filesystem update. Ignore malformed, incompatible, mismatched, and
oversized (over 16 KiB) records when listing. Storage access errors are
actionable errors and leave existing stars unchanged. On Windows, listing
retries transient entry-open conflicts like
[recent repositories](recent-repositories.md#recording-and-persistence).

## Web interface

Each skill card has a **Star** toggle button labeled with its path, with
`aria-pressed` reporting the state and visible **Star**/**Starred** text.
Pressing it stars or unstars the skill and keeps keyboard focus on the button.
Repeated presses are ignored while a request is pending.

The scan form shows **Starred skills**, newest first, with ties broken by
case-insensitive repository and then path. Each entry shows the name,
repository, and path, a commit-pinned **Source** link that follows the
[browser content protection](local-web-server.md#browser-content-protection)
rules, and an **Unstar** button. The list loads on page open, refreshes after
starring or unstarring and on window focus, and shows empty, disabled, and
unavailable states. Successful changes are announced in its status line.
Unstarring from the list moves focus to the list heading. A failed change keeps
the previous state and reports the error.

**Starred only** combines with the other
[result filters](web-results.md#search-and-filters), opens matching groups,
and resets with them on each new scan. When it removes the focused card, focus
moves to the filter. Starring is available while the page holds results and
does not start a scan or change the one-scan-per-server rule. When starring is
disabled, star buttons and **Starred only** are hidden.

## CLI

`skill-scanner starred` lists starred skills newest first with each name,
repository, path, and commit-pinned link, without contacting GitHub or requiring
credentials. Explain empty and disabled lists. `skill-scanner starred remove
OWNER/REPO PATH` accepts the same repository input forms as scanning and
persistently unstars the skill. Removing an absent star or a star in a missing
directory succeeds without creating it. Stars are added from the web interface.
Invalid repositories and paths exit 2; storage errors exit 1. Both commands
follow the CLI's stream, escaping, and broken-pipe conventions.

## Acceptance checks

Use temporary directories and mocked GitHub or browser scan responses only.
The [reproducible browser demo test](../../tests/e2e/README.md) covers starring,
the starred-only filter, stars across repositories, persistence after a
restart, a rescan, and unstarring from a card and from the list, with named
screenshot assertions.

- A new storage instance, CLI process, or web server sees prior stars. URL and
  case variants of a repository address the same star; paths are case-sensitive.
- Unstarring via the CLI, a card, or the list survives restart, affects only
  the chosen star, and allows the skill to be starred again.
- Invalid paths, commits, names, and repositories are rejected without writing
  files. Long names are shortened, and fully escaped maximum-length records fit
  the record limit.
- Corrupt, oversized, mismatched, and incompatible records do not break the
  remaining list. Concurrent stars of different skills are retained.
- Disabled storage creates no files; a regular file used as the directory is
  preserved and causes storage errors. Scans succeed regardless of starring.
- Web endpoints enforce the
  [local request protections](local-web-server.md#local-request-protection)
  and require no GitHub access.
- Browser checks exercise keyboard starring with retained focus, the filter,
  persistence, unstarring, and narrow-screen layout with mocked scan responses.
