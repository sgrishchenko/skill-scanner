# Organization scan

Status: implemented. Shared by the CLI and local web interface. Scans every
public, non-fork repository owned by one GitHub organization or user with the
same [discovery](skill-discovery.md), [metadata](skill-metadata.md),
[cache](analysis-cache.md), and [similarity](similarity-grouping.md) rules as a
single-repository scan.

## Input and listing

Accept one owner as `OWNER` or `https://github.com/OWNER`, optionally ending in
`/`. Apply GitHub's login rules (letters, digits, and single hyphens, at most 39
characters, no leading or trailing hyphen) and reject repository paths,
`/orgs/` URLs, credentials, ports, queries, and fragments. Invalid CLI input is
a usage error (exit 2); the web API returns HTTP 400 without echoing the input.
Organizations and user accounts use the same input.

List repositories with `GET /users/{owner}/repos` (owned repositories, sorted by
name, 100 per page) under the [repository access](repository-access.md) rules.
Request pages until one has fewer than 100 entries, reporting progress before
each page; more than 1,000 pages fail the scan. Validate every entry like user
input: the listed owner must match the requested owner case-insensitively and
each repository name must be valid. An inaccessible owner (HTTP 404), any other
listing failure, or an invalid entry fails the scan with no inventory.

- Scan only public repositories. Private entries are ignored.
- Skip forks and report how many were skipped; scan upstream repositories
  directly. Archived repositories are scanned.
- Scan a repository listed twice on shifting pages once.
- Order repositories case-insensitively by name. GitHub's owner spelling is
  used in results when any repository was listed.

## Repository scans

Scan repositories one at a time, following GitHub's guidance to avoid concurrent
API requests. The listing supplies each repository's details, so each scan
starts by resolving the default branch to a commit and then follows the usual
snapshot, [cache](analysis-cache.md#freshness-and-reuse), discovery, and
metadata steps. Cache entries are shared with single-repository scans.

Report progress before each repository with a one-based counter out of the
number of repositories to scan, then that repository's usual progress. A
repository's truncated recursive tree fails that repository instead of walking
every directory, because a very large repository could consume the API budget
for the whole organization; users can scan it individually.

| Repository outcome | Behavior |
| --- | --- |
| Completed with skills | Add its skills and pinned commit to the organization inventory |
| Completed with no skills | List it with its commit and zero skills |
| Empty repository | List it with no commit and zero skills |
| Disabled by GitHub | Record it as failed without requests |
| Repository-specific failure | Record its diagnostic, add no skills from it, and continue |
| Credential, rate-limit, or connection failure | Stop the whole scan with that diagnostic and the affected repository; return no inventory |

Credential rejection (HTTP 401), rate limits or denied access (HTTP 403/429),
long retry delays, and connection failures or timeouts affect every later
repository, so they stop the scan rather than marking every remaining
repository failed. Other HTTP failures, invalid or oversized responses,
truncated listings, and failed blob downloads affect only that repository.

## Inventory and aggregation

An organization inventory contains the owner, every scanned repository with its
commit, skill count, and optional failure diagnostic, the number of skipped
forks, and the skills of all completed repositories. Each skill path is prefixed
with `owner/repository/`, so paths stay unique, results sort by repository and
path, and search can match a repository. Links remain pinned to each
repository's scanned commit.

[Similarity grouping](similarity-grouping.md) and its statistics run once across
the combined skills, so matching skills in different repositories form one
group. Failed repositories contribute no skills and are never presented as
having zero skills.

An organization scan with any failed repository is incomplete: both interfaces
list the failed repositories with their diagnostics before the skills and label
the results incomplete. Organization scans do not add or update
[recent repositories](recent-repositories.md).

## Interfaces

- **CLI:** `skill-scanner scan-org OWNER` prints progress to stderr, with
  per-repository steps indented below each repository line, then the
  [organization report](cli-report.md#organization-report) on stdout. It exits 0
  when every repository completed, 1 when any repository failed (after printing
  the report) or the scan stopped, and 2 for invalid input.
- **Web:** the scan form's **Organization** mode submits to
  [`POST /api/scan-org`](web-scan-api.md#organization-scans) under the same
  one-scan-per-server rule, and the [results](web-results.md#organization-results)
  add a repository summary to the usual skill views.

## Acceptance checks

Use mocked GitHub or browser scan responses, without live GitHub access.

- Owner identifiers and URLs are accepted; repository paths, `/orgs/` URLs,
  credentials, and other unsupported inputs are rejected by both interfaces.
- Listing follows full pages, skips forks and private entries, removes
  duplicates, and orders repositories by name. Invalid or inaccessible listings
  return no inventory.
- Skills from several repositories appear with repository-prefixed paths and
  pinned links, and matching metadata groups skills across repositories.
- Empty repositories, repositories without skills, disabled repositories,
  truncated repositories, and other repository failures are reported distinctly;
  the remaining repositories still complete.
- A rate limit or credential failure stops the scan with the affected
  repository and no inventory, without requesting later repositories.
- Repository analyses reuse and populate the shared cache after commit
  revalidation. Organization scans leave recent repositories unchanged.
- The CLI exits 1 with a complete report when any repository failed. The web
  interface labels such results incomplete and lists the failures.
- Organization and repository scans share the one-scan-per-server limit.
