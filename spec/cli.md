# Skill Scanner CLI — initial specification

Status: initial draft. The product scope, Rust implementation language, and Linux,
macOS, and Windows targets are confirmed. Detailed behavior and other technical
recommendations remain proposals unless explicitly marked confirmed.

## Goal

Build a command-line application that scans one public GitHub repository and
lists its existing AI skills in the terminal, helping users discover what is
available and open the source files.

## Confirmed product scope

| Decision | Choice | Status |
| --- | --- | --- |
| What counts as a skill? | A directory containing a file named exactly `SKILL.md` | Confirmed |
| Which repositories are supported? | One public GitHub repository per invocation | Confirmed |
| How are results consumed? | Browse discovered skills in a terminal listing | Confirmed |

## Proposed first-version scope

- Accept a GitHub repository URL or `owner/repo` identifier.
- Scan the repository's default branch, resolving it to one commit for consistent results.
- Discover `SKILL.md` files recursively, including at the repository root and
  inside hidden directories. Discovery is not limited to known tool-specific paths.
- Show each discovered skill's name, description, repository path, and GitHub link.
- Report missing or unreadable metadata without hiding the discovered file.
- Read repository content without executing its scripts or skill instructions.

Private repositories, multi-repository or organization-wide scans, and other
agent configuration formats such as `AGENTS.md` and Cursor rules are outside
the confirmed scope. JSON export is deferred based on the terminal-browsing choice.

The proposed first version also excludes skill installation and execution,
quality or security ratings, Git history, submodule contents, and local repository inputs.

## Proposed command interface

The executable name `skill-scanner` is a placeholder.

```text
skill-scanner scan OWNER/REPO
skill-scanner scan https://github.com/OWNER/REPO
skill-scanner --help
skill-scanner --version
```

Branch, tag, and commit selection are deferred in the proposed first version.

## Proposed user flow and terminal report

1. Run `skill-scanner scan OWNER/REPO`.
2. The application resolves the default branch to a commit and scans its contents.
3. Read the skill names and descriptions, then follow a GitHub link to inspect a skill.

Illustrative output; the repository, skills, and commit placeholder are examples:

```text
Repository: example/agent-tools
Commit: <commit-sha>
Skills found: 2

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
For a completed scan with no results, print `Skills found: 0` and
`No SKILL.md files found.`

## Proposed discovery and reporting rules

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

## Proposed error behavior

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

## Acceptance examples for the proposed scope

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

| Decision | Choice or recommendation | Status |
| --- | --- | --- |
| Language | Rust | Confirmed |
| Installation | Standalone executable; a Cargo-based source installation can be added | Proposed |
| Supported operating systems | Linux, macOS, and Windows | Confirmed |
| Repository access | GitHub API, without requiring a local Git installation | Proposed |

If using the GitHub API, handle truncated directory listings explicitly so that
a large repository cannot silently produce an incomplete inventory. Allow an
optional `GITHUB_TOKEN` for higher public-repository API limits; this does not add private
repository support. Never include credentials in reports or diagnostics.

Use a Cargo binary crate. Keep command-line argument parsing, GitHub access,
skill discovery/metadata parsing, and report rendering separate so the behavior
can be tested using repository fixtures and mocked HTTP responses.

Before implementation, define network timeouts, retry behavior, file-size limits,
supported CPU architectures, and the release/install instructions for Rust.
