# CLI report

Status: implemented. Presents the shared [skill discovery](skill-discovery.md),
[metadata](skill-metadata.md), and [similarity grouping](similarity-grouping.md)
results in the terminal.

## Commands and user flow

The executable is named `skill-scanner`.

```text
skill-scanner scan OWNER/REPO
skill-scanner scan https://github.com/OWNER/REPO
skill-scanner serve
skill-scanner serve --port 8080
skill-scanner recent
skill-scanner recent remove OWNER/REPO
skill-scanner starred
skill-scanner starred remove OWNER/REPO PATH
skill-scanner --help
skill-scanner --version
```

Run `scan` with one supported [repository input](repository-access.md#repository-input).
The application resolves the default branch to a commit and scans its contents,
reusing a matching [cached analysis](analysis-cache.md) when available.
Read the skill names and descriptions, then follow a GitHub link to inspect a
source file. `serve` launches the [local web server](local-web-server.md).
`recent` lists successful prior scans without GitHub access; `recent remove`
forgets an entry. See [recent repositories](recent-repositories.md) for storage,
ordering, and removal semantics. History access errors exit 1; successful list
and removal operations exit 0, and invalid repository inputs exit 2.
`starred` lists starred skills and `starred remove` unstars one, with the same
exit codes; see [starred skills](starred-skills.md#cli).
Branch selection and CLI JSON export remain deferred.

## Output streams and progress

Render the complete inventory to stdout only after the scan succeeds.
Operational diagnostics, progress, warnings, and errors go to stderr. Metadata
warnings identify the affected file and field.

Flush plain-text [progress](skill-discovery.md#progress) before each operation,
including fallback directory paths and each skill's path with a one-based
counter out of the discovered total.
On cache hits, print the reused commit after resolution, then render the usual
report and warnings without per-file scan progress.

Use readable plain text without requiring color, a TTY, or a pager. Escape
control characters from repository content, including terminal escape
sequences. Redirected output remains readable. Broken stdout pipes exit
successfully.

## Terminal inventory

Include the repository, scanned commit, number of discovered skills, and all
[similarity statistics](similarity-grouping.md#ordering-and-statistics). Before
the full inventory, list each similar group's representative name, count,
percentage, warning count, and member paths in the shared group ordering.
The full inventory remains sorted by repository-relative path, with each
skill's name, description, path, and commit-pinned GitHub link.

Illustrative output; the repository, skills, and commit placeholder are examples:

```text
Repository: example/agent-tools
Commit: <commit-sha>
Skills found: 2
Similar groups: 0
Skills in similar groups: 0 (0.0%)
Standalone skills: 2
Largest similar group: 0
Skills with warnings: 0

code-review
  Description: Review changes for correctness and maintainability.
  Path: skills/code-review/SKILL.md
  Link: https://github.com/example/agent-tools/blob/<commit-sha>/skills/code-review/SKILL.md

release-notes
  Description: Draft release notes from a set of changes.
  Path: skills/release-notes/SKILL.md
  Link: https://github.com/example/agent-tools/blob/<commit-sha>/skills/release-notes/SKILL.md
```

For a completed scan with no results, print `Skills found: 0` and
`No SKILL.md files found.` An empty repository additionally reports that there
is no commit to scan. Failed or incomplete scans produce a diagnostic without
a completed inventory.

## Exit codes

| Exit code | Meaning |
| --- | --- |
| `0` | Completed scan, including zero skills or metadata warnings; also help/version output and broken stdout pipes |
| `1` | Scan or report could not be completed; also web server startup failures |
| `2` | Invalid command usage or repository input |

## Acceptance checks

- Successful scans show every source file with its metadata, path, and pinned
  link, including duplicate names and files with metadata warnings.
- Similar-group summaries precede the full inventory and agree with the web
  interface's aggregation.
- Progress appears before the corresponding operation, with skill counters
  and fallback directory paths where applicable.
- Reports go to stdout; progress, warnings, and errors go to stderr. Redirecting
  stdout preserves a readable report, and repository control characters are
  escaped.
- Zero results and empty repositories succeed; failed scans do not emit an
  inventory. Usage errors, help/version, and broken pipes follow the exit table.
