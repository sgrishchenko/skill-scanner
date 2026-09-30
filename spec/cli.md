# Skill Scanner CLI specification

Status: implemented. This document records the CLI behavior and shared scanning
rules. The local browser interface is specified in [web.md](web.md). Technical
choices and limits are recorded in [implementation.md](implementation.md).

## Goal

Build a command-line application that scans one public GitHub repository and
lists its existing AI skills in the terminal, helping users discover what is
available and open the source files.

## Confirmed product scope

| Decision | Choice | Status |
| --- | --- | --- |
| What counts as a skill? | A directory containing a file named exactly `SKILL.md` | Confirmed |
| Which repositories are supported? | One public GitHub repository per invocation | Confirmed |
| How are results consumed? | Browse discovered skills in a terminal listing or local web interface | Implemented |

## Implemented scan scope

- Accept a GitHub repository URL or `owner/repo` identifier.
- Scan the repository's default branch, resolving it to one commit for consistent results.
- Discover `SKILL.md` files recursively, including at the repository root and
  inside hidden directories. Discovery is not limited to known tool-specific paths.
- Show each discovered skill's name, description, repository path, and GitHub link.
- Report missing or unreadable metadata without hiding the discovered file.
- Read repository content without executing its scripts or skill instructions.

Private repositories, multi-repository or organization-wide scans, and other
agent configuration formats such as `AGENTS.md` and Cursor rules are outside
the supported scope. A CLI JSON export mode remains deferred; the web interface's
internal JSON transport does not change the terminal report format.

This version also excludes skill installation and execution,
quality or security ratings, Git history, submodule contents, and local repository inputs.

## Command interface

The executable is named `skill-scanner`.

```text
skill-scanner scan OWNER/REPO
skill-scanner scan https://github.com/OWNER/REPO
skill-scanner serve
skill-scanner serve --port 8080
skill-scanner --help
skill-scanner --version
```

Branch, tag, and commit selection are deferred. `serve` runs the local browser
interface; see [web.md](web.md) for its command, interaction, and API contract.

## User flow and terminal report

1. Run `skill-scanner scan OWNER/REPO`.
2. The application resolves the default branch to a commit and scans its contents.
3. Read the skill names and descriptions, then follow a GitHub link to inspect a skill.

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

Write the report to stdout and operational diagnostics to stderr. Use readable
plain text without requiring color, an interactive terminal, or a pager.
Include the repository, scanned commit, and number of discovered skills.
Include similar-group counts, grouped skills and their percentage of all skills,
standalone skills, largest similar-group size (zero if none), and skills with
warnings. Before the full inventory, list each similar group's representative
name, count, percentage, warning count, and member paths. Order groups by size
descending, breaking ties by the first member path; order members by path.
For a completed scan with no results, print `Skills found: 0` and
`No SKILL.md files found.`

## Discovery and reporting rules

1. Treat each regular file named exactly `SKILL.md` as a candidate. Do not follow symlinks.
2. Identify a candidate by its repository-relative file path; duplicate names
   in different directories remain separate results.
3. Read `name` and `description` from YAML front matter when available.
   Missing or invalid fields produce a warning. Discovery alone does not certify
   that a skill is valid or compatible with a particular agent.
4. Use the containing directory name as the display-name fallback, or the
   repository name for a root-level `SKILL.md`. Leave unavailable descriptions empty.
5. Sort results by repository-relative path so repeated scans of the same commit
   produce a stable ordering. Link to files at the scanned commit.
6. Distinguish a successful scan with zero candidates from a failed or incomplete scan.
7. Group skills whose valid names or descriptions match after lowercasing and
   replacing punctuation/whitespace runs with a single space. Compare names only
   to names and descriptions only to descriptions. Ignore unavailable fields,
   fallback names, and normalized empty values. Merge connected matches so each
   skill belongs to exactly one group, including standalone skills. Similar
   groups contain two or more members. This is metadata similarity, not a claim
   of identical file contents or semantic equivalence.

## Error behavior

| Exit code | Meaning |
| --- | --- |
| `0` | Completed scan, including zero skills or metadata warnings; also help/version output |
| `1` | Scan could not be completed |
| `2` | Invalid command usage or repository input |

- Inaccessible repositories, network failures, rate limits, and incomplete scans
  produce a clear diagnostic with a suggested next step where possible.
- Metadata warnings identify the affected file and field.
- Do not present partial results as a completed inventory.
- An empty repository is a successful scan with zero skills; report that there
  is no commit to scan instead of inventing a commit identifier.

## Acceptance examples

- A repository with skills in two nested directories produces two results with
  their paths and links.
- A root-level skill and a skill inside a hidden directory are both discovered.
- Two skills with the same name and different paths are both reported.
- A `SKILL.md` with missing or malformed metadata is listed with a warning.
- A repository with no `SKILL.md` files reports zero results and succeeds.
- An inaccessible repository fails clearly instead of reporting zero results.
- An interrupted or truncated scan is never presented as complete.
- Repository scripts and skill instructions are never executed during a scan.
- Repeated scans of the same commit list skills in the same order.
- The report remains readable when stdout is redirected to a file.
- A public repository can be scanned without configuring credentials, subject
  to GitHub's anonymous access limits.

## Technical design

| Decision | Choice | Status |
| --- | --- | --- |
| Language | Rust | Confirmed |
| Installation | Standalone executable and `cargo install --path . --locked` | Implemented |
| Supported operating systems | Linux, macOS, and Windows | Confirmed |
| Repository access | GitHub API, without requiring a local Git installation | Implemented |

Handle truncated GitHub API directory listings explicitly so that
a large repository cannot silently produce an incomplete inventory. Allow an
optional `GITHUB_TOKEN` for higher public-repository API limits; this does not add private
repository support. Never include credentials in reports or diagnostics.

Use a Cargo binary crate. Keep command-line argument parsing, GitHub access,
skill discovery/metadata parsing, and report rendering separate so the behavior
can be tested using repository fixtures and mocked HTTP responses.

Network timeouts, retry behavior, file-size limits, supported CPU architectures,
and release/install instructions are fixed in [implementation.md](implementation.md).
