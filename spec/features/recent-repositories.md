# Recent repositories

Status: implemented. The CLI and web interface share a persistent list of
successfully scanned public GitHub repositories.

## Recording and persistence

- Save a repository only after a complete successful scan, including cache
  hits, metadata warnings, zero skills, and empty repositories. A failed or
  partial scan neither creates an entry nor updates an existing entry.
- Keep one entry per case-insensitive owner/repository. All supported input
  forms share that entry. A subsequent successful scan updates its spelling,
  completion time, and skill count. List newest first, with case-insensitive
  repository order breaking ties in completion time.
- Store only the normalized repository, last successful completion time (UTC
  milliseconds since the Unix epoch), skill count, and storage format version.
  Do not save credentials, raw content, errors, or full scan results here.
- Share entries between CLI invocations, tabs, server restarts, and ports under
  the same user and history configuration. Read disk on each list request.
  History is independent of the [analysis cache](analysis-cache.md); deleting
  or disabling the cache does not remove the recent list. Existing cache files
  are not imported; repositories are recorded on their next successful scan.

| Platform | Default history directory |
| --- | --- |
| Linux and other Unix systems | `$XDG_STATE_HOME/skill-scanner/recent` when absolute and nonempty; otherwise `$HOME/.local/state/skill-scanner/recent` |
| macOS | `$HOME/Library/Application Support/skill-scanner/recent` |
| Windows | `%LOCALAPPDATA%\skill-scanner\recent` |

`SKILL_SCANNER_HISTORY_DIR` overrides the entire directory. Relative values
resolve against the process working directory; an empty value disables history.
If no platform directory can be resolved, history is disabled. Create the
directory only when recording a successful scan. Entries remain until removed.

Publish each repository's versioned JSON file atomically. Concurrent writes
to different repositories preserve both entries; concurrent operations on the
same repository use the last filesystem update. Ignore malformed,
incompatible, mismatched, and oversized (over 4 KiB) records when listing.
Storage access errors are actionable list/removal errors. A recording failure
warns without failing an otherwise successful scan or hiding its results.

## Listing and removal

`skill-scanner recent` lists saved repository identifiers and skill counts,
newest first, without contacting GitHub or requiring credentials. Explain empty
and disabled history. `skill-scanner recent remove OWNER/REPO` accepts the same
repository input forms as scanning and persistently removes the matching entry.
Removing an absent entry or an entry in a missing history directory succeeds
without creating the directory. A history path that is a regular file is a
storage error on every supported platform. Both commands follow the CLI's
stream, escaping, broken-pipe, and exit-code conventions.

The web scan form shows **Recent repositories**, each repository's skill count
and last successful scan time, and a **Remove** button. Selecting a repository
fills and focuses the scan field; submission performs the usual online scan
and cache validation. The list loads on page open, refreshes after scans and
removal, and refreshes on window focus or **Refresh list** so scans from other
tabs and CLI runs can be seen. Show empty, disabled, and unavailable states.

Removal affects only the recent list. It keeps the current displayed inventory,
cached analysis, and remote GitHub repository. A later successful scan adds
the repository again, including a scan already running in another tab/process.
Disable repository selection and removal while this page is scanning. Failed
removal keeps the entry and reports the error. A recording warning is visible
alongside the completed scan. Use accessible buttons and inert DOM text, with
wrapping repository names and a scrollable list on narrow screens.

Full inventories remain in page memory and disappear on reload. Recent entries
do not enable offline result browsing or change the one-scan-per-server rule.

## Acceptance checks

Use temporary directories and mocked GitHub or browser scan responses only.

- A new storage instance, CLI process, or web server sees successful prior
  scans, including empty repositories and cache hits. New scans move entries
  to the top, and URL/case variants update the same entry.
- Failed scans preserve the list. Unavailable history warns while returning
  a complete scan. Disabled history creates no files.
- Removal via CLI or browser survives reload/restart, affects only the chosen
  entry, and allows it to return after a successful new scan.
- Removal from a missing history directory succeeds without creating it.
  A regular file used as the history path is preserved and causes a CLI failure
  (exit 1) and an HTTP 500 removal response, including on Windows.
- Concurrent repository writes retain valid records. Corrupt, oversized, and
  incompatible entries do not break the remaining list.
- Listing and removal require no GitHub access; web endpoints enforce the
  [local request protections](local-web-server.md#local-request-protection).
- Browser checks exercise reload, selection, removal, empty states, failure
  recovery, disabled controls while scanning, keyboard access, and mobile
  layout with mocked scan responses.
