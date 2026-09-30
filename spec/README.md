# Skill Scanner specification

Status: implemented. Skill Scanner discovers the existing AI skills in one
public GitHub repository so users can browse them in a terminal or local web
interface and open their source files. A skill is a directory containing a
regular file named exactly `SKILL.md`.

## Feature specifications

Each feature document owns its behavior and acceptance checks. Shared scanning
rules apply to both interfaces; interface documents link to those rules.

| Feature | Behavior covered |
| --- | --- |
| [Repository access](features/repository-access.md) | Accepted inputs, public GitHub access, credentials, timeouts, retries, and API limits |
| [Skill discovery](features/skill-discovery.md) | Default-branch snapshot, recursive discovery, progress, ordering, and scan completeness |
| [Analysis cache](features/analysis-cache.md) | Persistent shared analyses, commit validation, invalidation, storage, and cache failures |
| [Recent repositories](features/recent-repositories.md) | Shared persistent recent list, completion times, selection, and removal |
| [Skill metadata](features/skill-metadata.md) | YAML fields, file limits, warnings, and display fallbacks |
| [Similarity grouping](features/similarity-grouping.md) | Metadata matching, connected groups, ordering, and statistics |
| [CLI report](features/cli-report.md) | Commands, terminal inventory, stdout/stderr, and exit codes |
| [Local web server](features/local-web-server.md) | Launch, embedded assets, loopback access, and browser security |
| [Web scan](features/web-scan.md) | Form submission, live progress, completion, failures, and scan lifecycle |
| [Web results](features/web-results.md) | Summary, skill cards, grouped/all views, search, filters, and accessibility |
| [Web scan API](features/web-scan-api.md) | Routes, request validation, streaming events, and inventory schema |
| [Installation and releases](features/installation-and-releases.md) | Source installation, supported platforms, packages, and release delivery |

[Implementation decisions](implementation.md) records the architecture and
verification approach supporting these features.

## Product boundaries

- Scan one public GitHub repository per invocation, using its default branch.
- Discover skills at the repository root and recursively in all directories,
  including hidden directories, without restricting discovery to tool-specific
  paths.
- Show each discovered file's name, description, repository path, and source
  link, retaining files whose metadata is missing or unreadable.
- Read repository content without executing scripts or skill instructions.
  Discovery does not certify a skill's validity, compatibility, or safety.

Private repositories, multi-repository and organization-wide scans, local
repository inputs, branch/tag/commit selection, Git history, submodule contents,
and other agent configuration formats such as `AGENTS.md` and Cursor rules are
outside the supported scope. Skill installation/execution and quality or
security ratings are also excluded.

Hosted deployment, accounts, hosted authentication, a public bind option, full
saved scan reports, and report export remain deferred. The internal web JSON
transport is not a general export API and does not add a CLI JSON mode.
The [analysis cache](features/analysis-cache.md) speeds up repeated scans after
online commit validation. The
[recent repository list](features/recent-repositories.md) separately remembers
successful scans and supports removal.
