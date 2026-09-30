# Skill discovery

Status: implemented. Shared by the CLI and local web interface.
Repositories follow the [repository access](repository-access.md) contract.

## Snapshot and candidates

1. Resolve the repository's default branch once to one commit and root tree.
   Use only tree/blob object IDs afterward so the inventory is consistent even
   if the branch changes during the scan.
2. Discover regular files named exactly `SKILL.md` recursively, including the
   repository root and hidden directories. Discovery is not limited to known
   tool-specific paths. Regular modes `100644` and `100755` qualify; skip
   symlinks and submodules.
3. Try a recursive Git tree request. If it is truncated, discard it and walk
   nonrecursive directory trees. Fail if even a nonrecursive listing is
   truncated; a large repository must never silently produce an incomplete
   inventory.
4. Identify each candidate by its repository-relative file path. Duplicate
   names in different directories remain separate results.
5. Read each candidate's [metadata](skill-metadata.md) without executing
   repository scripts or skill instructions.
6. Sort results by repository-relative path. Link to files at the scanned
   commit with URL-encoded GitHub links. Repeated scans of the same commit
   produce the same ordering.

## Progress

Report progress before resolving the repository, resolving the default branch,
discovering files, walking each directory during a truncated-tree fallback,
and reading each skill. Individual skill progress includes its path and a
one-based counter out of the discovered total.

The counter describes the file about to be read. The
[CLI](cli-report.md#output-streams-and-progress) flushes progress to stderr;
the [web scan](web-scan.md#submission-and-progress) uses the same events to show
live progress without exposing a partial inventory.

## Scan outcomes

| Outcome | Behavior |
| --- | --- |
| Completed with candidates | Return the full inventory, including files with metadata warnings |
| Completed with no candidates | Return zero skills and retain the scanned commit |
| Empty repository | Succeed with zero skills and no commit; do not invent a commit identifier |
| Failed or incomplete scan | Show a diagnostic and no completed inventory |

Metadata warnings do not fail discovery. Network, access/rate-limit,
truncation, API listing, and blob-download failures do. Do not present partial
results as a completed inventory or failures as zero skills.

## Acceptance checks

- Skills in two nested directories produce two results with their paths and
  commit-pinned links.
- A root-level skill and a skill inside a hidden directory are both discovered.
- Two skills with the same name and different paths remain separate results.
- Symlinks and submodule contents are skipped.
- Repeated scans of the same commit list skills in the same order.
- A repository with no `SKILL.md` files succeeds with zero results; an empty
  repository additionally has no commit to scan.
- Truncated recursive trees trigger a complete directory walk. An interrupted
  or still-truncated scan is never presented as complete.
- Repository scripts and skill instructions are never executed during a scan.
