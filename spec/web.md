# Local web interface specification

Status: implemented. The selected product shape is a local interface launched
with `skill-scanner serve`. It extends the CLI with browser-based discovery and
uses the same scanner and [scanning rules](cli.md).

## Scope and launch

```console
skill-scanner serve
skill-scanner serve --port 8080
```

The default URL is `http://127.0.0.1:3000`. The server binds only to IPv4 loopback
and prints the actual URL to stderr. `--port 0` selects an available port;
non-numeric ports and values outside 0–65535 are usage errors (exit 2). An
occupied port or startup failure produces a diagnostic and exit 1. Open the
printed URL in a current browser with JavaScript enabled. Ctrl+C stops the
process; the server does not launch a browser automatically.

All assets ship in the executable. Source builds use Cargo; no frontend build
or additional runtime is needed. The same executable continues to support
`scan` with its existing output and exit-code contract.

Hosted deployment, accounts, history, installation/execution, local repository
input, branch selection, multi-repository scans, and report export are deferred.
There is no public bind option or authentication system for hosted use.

## User flow

1. Enter one public `owner/repo` or HTTPS GitHub repository URL. The web form trims
   surrounding whitespace, then applies the CLI's repository validation. Example
   buttons fill the field; scanning starts only on form submission.
2. Submit **Scan repository**. Clear the old inventory and filters. Disable the
   form during the request and announce live progress: repository resolution,
   default branch, discovery, fallback directories, and individual skill paths.
   Show an indeterminate progress bar until the candidate count is known. Skill
   counters describe the file about to be read, so the progress bar counts only
   files already read.
3. On completion, display the repository, full commit in the commit link's
   accessible label/title (shortened visually), total skills, and the number of
   skills with metadata warnings. Show similar-group counts, grouped skills and
   their percentage of all skills, standalone skills, and largest similar-group
   size (zero when there are no similar groups). These statistics include the
   entire scan and remain visible for successful empty scans with zero values.
4. Each card shows the skill's name, description, path, and a commit-pinned GitHub
   source link. Missing descriptions say “No description available.” Expand
   metadata warnings to read field-specific messages. Names use the scanner's
   directory/repository fallback. Each source file remains individually accessible.
   Default to **Grouped skills**, with similar groups largest first and ties
   broken by their first member path. Show size, percentage of all skills, and
   skills with warnings for each group. Expand a group to inspect its member
   cards sorted by path. Standalone skills appear as individual cards. **All
   skills** displays every card sorted by path. Explain the shared
   [metadata matching rules](cli.md#discovery-and-reporting-rules).
5. Search names, descriptions, and paths by case-insensitive substring, and
   optionally show only similar skills or skills with warnings. Filters combine
   with AND and run locally in both views. Apply filters to individual members;
   open matching groups automatically when searching or filtering by warnings,
   and state how many members remain visible. A group remains similar even when
   filters leave only one visible member. Show visible/total skill and group
   counts and a clear-filters action when no cards match. Summary and group
   statistics always describe the complete scan. Reset filters and the view on
   each new scan.
6. Start another scan using the same form. The app retains no scan history.

## States and failure behavior

| State | Behavior |
| --- | --- |
| Initial | Explain where results will appear; show no fabricated inventory |
| Scanning | Show actual progress; disable duplicate submission; show no partial inventory |
| Completed with skills | Render the complete inventory, filters, and source links |
| Completed with no skills | Say “No SKILL.md files found”; retain the scanned commit |
| Empty repository | Say the repository is empty and there is no commit to scan |
| Filters match nothing | Keep the inventory and show a clear-filters action |
| Invalid input | Show an actionable validation error without echoing rejected input |
| Failed scan | Show the scanner's diagnostic; restore the form; show no inventory |
| Broken stream/server unavailable | Explain that the scan did not finish and offer retry via the form |
| Another tab is scanning | Show the busy diagnostic; do not queue a second scan |

Metadata warnings are successful discovery results. Network, access/rate-limit,
truncation, or blob failures remain failures and cannot be shown as zero skills.
An HTTP success status alone is insufficient: the browser requires a complete
event before labeling a scan complete. Closing/reloading the page discards its
inventory. A disconnected scan finishes in the server, holding the single scan
slot until completion; the next request can receive a busy response meanwhile.

## Internal HTTP contract

The HTTP API exists for this interface and is not a general JSON export API.

| Route | Purpose |
| --- | --- |
| `GET /` | Embedded HTML |
| `GET /app.css`, `GET /app.js`, `GET /favicon.svg` | Embedded assets with appropriate MIME types |
| `POST /api/scan` | Validate one repository and stream scan events |

POST uses `Content-Type: application/json` and `X-Skill-Scanner: 1` with a body
such as `{"repository":"example/skills"}`. Unknown fields are rejected. Bodies
are limited to 4 KiB. Before streaming, errors are JSON with a `message`:

| Status | Meaning |
| --- | --- |
| 400 | Invalid repository or malformed JSON |
| 403 | Invalid local Host/Origin or missing/invalid action header |
| 404 / 405 | Unknown route / unsupported method |
| 409 | Another scan is active |
| 413 | Body exceeds the input limit |
| 415 | Missing or unsupported JSON content type |
| 422 | Invalid JSON request shape, missing field, or unknown field |

Accepted requests return HTTP 200 with
`Content-Type: application/x-ndjson; charset=utf-8`. Each newline terminates a
JSON object. Progress arrives before the corresponding GitHub operation:

```json
{"type":"progress","message":"Resolving repository: example/skills","current":null,"total":null}
{"type":"progress","message":"Scanning skill [1/2]: skills/review/SKILL.md","current":1,"total":2}
```

`current` and `total` are set only for individual skills. A successful scan ends
with `{"type":"complete","inventory":{...}}`. The inventory has `repository`
(normalized string), `commit` (full SHA or null for an empty repository), and
`skills`, and `aggregation`. Each skill contains `name`, `description`, `path`, `link`, and `warnings`;
each warning has `field` and `message`. A failed scan ends with
`{"type":"error","message":"..."}` and no inventory. JSON encoding handles
embedded newlines; clients must handle event boundaries split across chunks.

`aggregation.statistics` contains integer counts: `total_skills`,
`similar_groups`, `grouped_skills`, `standalone_skills`, `largest_group`, and
`skills_with_warnings`. `aggregation.groups` contains every group, including
singletons, ordered largest first then by first member path. Each group has
`skill_indices` (zero-based indices into `skills`, sorted by path) and
`skills_with_warnings`. Each skill index occurs in exactly one group. Empty
inventories have no groups and all-zero statistics. Percentages use the full
scan's skill count as the denominator, with zero for an empty scan.

## Local access and accessibility

The server keeps GitHub credentials in the process environment/client. No token
field or browser token storage is provided. Host and Origin checks, a custom
action header, no CORS, and a restrictive CSP protect the local interface.
Repository strings are rendered as text; HTML and Markdown are never executed.
External links are restricted to HTTPS GitHub and open in a new tab with opener
access disabled. The product explains that discovery does not certify safety
or compatibility and that it never installs or runs skills.

The interface uses labeled controls, semantic headings, keyboard operation,
visible focus, a skip link, live progress/error announcements, and reduced-motion
styles. Layout adapts to narrow screens without horizontal scrolling. It works
without external images, fonts, analytics, or frontend network dependencies.

## Acceptance checks

- The installed binary serves the interface from any working directory.
- Default, custom, available (`0`), invalid, and occupied ports behave as specified.
- A successful scan renders pinned links, duplicate names, and visible metadata
  warnings; repository-controlled markup stays inert text.
- Progress reaches the page before completion; incomplete streams show errors.
- Search and warnings filters combine correctly and can be cleared.
- Grouped/all views retain every source. Similarity statistics and group counts
  agree with the CLI, and remain stable when filters reduce visible members.
- Similar-only, warning, and search filters combine in both views; missing
  metadata does not produce false similarity groups.
- Zero results, empty repositories, invalid input, busy responses, and upstream
  failures have distinct behavior with no stale or partial inventory.
- Multiple tabs cannot start concurrent scans. Capacity is released after a
  disconnected scan finishes.
- Foreign origins/hosts, unmarked requests, and oversized bodies are rejected.
- Desktop and narrow-screen layouts support the same keyboard-accessible flow.
- Existing CLI scan behavior and tests continue to pass.
