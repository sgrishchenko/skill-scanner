# Web scan

Status: implemented. The [local web server](local-web-server.md) uses the same
[scanner](skill-discovery.md) as the CLI and the
[web scan API](web-scan-api.md) for streaming updates.

## Submission and progress

1. Enter one public `owner/repo` or HTTPS GitHub repository URL. Trim surrounding
   whitespace before applying the shared
   [repository validation](repository-access.md#repository-input). Example
   buttons fill the field; scanning starts only on form submission.
2. Submit **Scan repository**. Clear the previous inventory and filters, reset
   the results view, and disable the form during the request.
3. Announce actual progress for repository resolution, default branch,
   discovery, fallback directories, and individual skill paths. Show an
   indeterminate progress bar until the candidate count is known. Skill
   counters describe the file about to be read, so the progress bar counts only
   files already read.
   A [cache hit](analysis-cache.md#progress-and-web-lifecycle) announces the
   reused commit after online validation, with indeterminate progress until
   completion and no per-skill counters.
4. Require a completion event before presenting the full
   [results](web-results.md). An HTTP success status alone is insufficient.
   Never show a partial inventory.
5. Restore the form after completion or failure so users can start another scan.

## States and failure behavior

| State | Behavior |
| --- | --- |
| Initial | Explain where results will appear; show no fabricated inventory |
| Scanning | Show actual progress; disable duplicate submission; show no partial inventory |
| Completed with skills | Render the complete inventory, filters, and source links |
| Completed with no skills | Say “No SKILL.md files found”; retain the scanned commit |
| Empty repository | Say the repository is empty and there is no commit to scan |
| Invalid input | Show an actionable validation error without echoing rejected input |
| Failed scan | Show the scanner's diagnostic; restore the form; show no inventory |
| Broken stream/server unavailable | Explain that the scan did not finish and offer retry via the form |
| Another tab is scanning | Show the busy diagnostic; do not queue a second scan |

Metadata warnings are successful discovery results. Network, access/rate-limit,
truncation, or blob failures remain failures and cannot be shown as zero skills.
A failed scan cannot leave a stale inventory labeled complete. A successful
scan whose filters match nothing follows the separate
[results filtering](web-results.md#search-and-filters) behavior.

## Concurrency and state

Allow one scan per server across all tabs. Additional scans receive HTTP 409;
there is no queue or cancel control. Disconnecting drops the response but does
not cancel GitHub work: the worker finishes its scan before releasing capacity.
A request during that time can still receive a busy response.

Keep only the current inventory in page memory. Closing or reloading the page
discards the displayed results. The shared [analysis cache](analysis-cache.md)
persists complete analyses on disk, including successful disconnected scans;
another submission validates the current commit before reuse. There are no
saved-scan browsing controls, history, server job IDs, polling endpoints, or
database.

## Acceptance checks

- Example buttons populate the input without starting a scan; submission clears
  old results and filters and prevents duplicate requests.
- Progress reaches the page before completion. An incomplete stream shows an
  error even if its HTTP status was successful.
- Zero results, empty repositories, invalid input, busy responses, and upstream
  failures have distinct behavior with no stale or partial inventory.
- Multiple tabs cannot start concurrent scans. Capacity is released after a
  disconnected scan finishes.
- Starting another scan resets the view and filters; reloading clears results.
- A cached scan announces reuse and renders the same full results and warnings
  as a fresh scan; reloading still requires another submission and validation.
