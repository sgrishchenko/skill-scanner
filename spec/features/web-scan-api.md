# Web scan API

Status: implemented. This internal HTTP contract serves the
[local web interface](local-web-server.md); it is not a general JSON export API.

## Routes

| Route | Purpose |
| --- | --- |
| `GET /` | Embedded HTML |
| `GET /app.css`, `GET /app.js`, `GET /favicon.svg` | Embedded assets with appropriate MIME types |
| `POST /api/scan` | Validate one repository and stream scan events |
| `POST /api/scan-org` | Validate one organization or user and stream [organization scan](#organization-scans) events |
| `GET /api/recent` | List saved successful scans, newest first |
| `POST /api/recent/remove` | Remove one repository from the recent list |

## Request validation

Use one POST per scan with `Content-Type: application/json` and
`X-Skill-Scanner: 1`. The JSON body contains one repository:

```json
{"repository":"example/skills"}
```

Reject unknown fields and bodies larger than 4 KiB. Apply shared
[repository validation](repository-access.md#repository-input) and
[local Host/Origin checks](local-web-server.md#local-request-protection).
Before streaming, errors are JSON with a `message`:

| Status | Meaning |
| --- | --- |
| 400 | Invalid repository or malformed JSON |
| 403 | Invalid local Host/Origin or missing/invalid action header |
| 404 / 405 | Unknown route / unsupported method |
| 409 | Another scan is active |
| 413 | Body exceeds the input limit |
| 415 | Missing or unsupported JSON content type |
| 422 | Invalid JSON request shape, missing field, or unknown field |

## Recent repositories

`GET /api/recent` returns `{"enabled":true,"repositories":[...]}`. Each entry
contains `repository` (normalized string), `scanned_at` (UTC milliseconds since
the Unix epoch), and `skill_count` (integer). Disabled history returns
`enabled: false` and an empty list. These reads do not contact GitHub.

`POST /api/recent/remove` uses the same JSON body and validation as `/api/scan`,
including the action header, content type, input limits, and Host/Origin checks.
It returns HTTP 204 when removal succeeds or the entry is already absent.
List/removal storage failures return HTTP 500 with an actionable JSON `message`.
These routes neither start a scan nor acquire its semaphore. See
[recent repositories](recent-repositories.md) for persistence and concurrency.

## Streaming events

Accepted requests return HTTP 200 with
`Content-Type: application/x-ndjson; charset=utf-8`. Each newline terminates a
JSON object. Operation progress arrives before the corresponding GitHub request:

```json
{"type":"progress","message":"Resolving repository: example/skills","current":null,"total":null}
{"type":"progress","message":"Scanning skill [1/2]: skills/review/SKILL.md","current":1,"total":2}
```

`current` and `total` are set only for individual skills, following the shared
[progress semantics](skill-discovery.md#progress). A successful scan ends with
`{"type":"complete","inventory":{...}}`. Only this final event contains an
inventory. A failed scan ends with `{"type":"error","message":"..."}` and
no inventory. If saving a completed scan to recent repositories fails, emit
`{"type":"history_warning","message":"..."}` before the completion event.
This warning does not fail or hide the inventory. JSON encoding handles embedded newlines; clients must handle
event boundaries split across chunks.

After repository and commit resolution, an [analysis cache](analysis-cache.md)
hit emits a progress message `Using cached analysis for commit: <full-sha>` with
null `current` and `total`, then the usual completion inventory. It emits no
discovery or individual-skill progress. Cached scans use the same request and
inventory schema and retain `Cache-Control: no-store` for HTTP responses.

An HTTP success status without a completion event is not a successful scan.
[Web scan lifecycle](web-scan.md#concurrency-and-state) defines concurrency,
disconnection, and page state.

## Organization scans

`POST /api/scan-org` uses the same headers, 4 KiB limit, Host/Origin checks,
error statuses, and one-scan semaphore as `/api/scan`, with one owner field:

```json
{"organization":"example"}
```

It applies the [organization input](organization-scan.md#input-and-listing)
validation and streams the same event types. Listing progress has null counters.
Each repository's progress and all of its nested progress carry that
repository's one-based `current` and the repository `total`; nested messages are
prefixed with the repository, such as
`example/tools: Scanning skill [1/2]: skills/review/SKILL.md`. A scan stopped by
credential, rate-limit, or connection failures ends with an `error` event.
History warnings do not occur because organization scans do not record recent
repositories.

The `complete` event's inventory contains `organization` (GitHub's spelling of
the owner), `repositories`, `skipped_forks`, `skills`, and `aggregation`. Each
repository has `repository`, `commit` (null when empty or failed),
`skill_count`, and `error` (null unless that repository failed). Skills and
aggregation use the schema below; skill paths are prefixed with
`owner/repository/` and aggregation spans all repositories.

## Inventory and aggregation

The inventory contains `repository` (normalized string), `commit` (full SHA or
null for an empty repository), `skills`, and `aggregation`. Each skill contains
`name`, `description`, `path`, `link`, and `warnings`; each warning has `field`
and `message`.

`aggregation.statistics` contains integer counts: `total_skills`,
`similar_groups`, `grouped_skills`, `standalone_skills`, `largest_group`, and
`skills_with_warnings`.

`aggregation.groups` contains every group, including singletons, ordered
largest first then by first member path. Each group has `skill_indices`
(zero-based indices into `skills`, sorted by path) and `skills_with_warnings`.
Each skill index occurs in exactly one group. Empty inventories have no groups
and all-zero statistics. Percentages use the full scan's skill count as the
denominator, with zero for an empty scan. See
[similarity grouping](similarity-grouping.md) for the matching and counting rules.

## Acceptance checks

- Input, JSON shape, content type, body limits, Host/Origin, and action-header
  validation return the specified pre-stream statuses.
- Accepted scans stream progress before completion and return an inventory
  only in a completion event; scanner failures emit an error event.
- Clients handle JSON newlines and event boundaries split across chunks.
- Inventory responses preserve warnings, empty repositories, and every skill's
  membership in exactly one aggregation group.
- Concurrent scans receive HTTP 409, including while a disconnected worker
  finishes its scan, across repository and organization routes.
- Organization requests validate the owner field and stream repository-counted
  progress before one combined inventory with per-repository outcomes.
