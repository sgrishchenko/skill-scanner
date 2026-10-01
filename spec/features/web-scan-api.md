# Web scan API

Status: implemented. This internal HTTP contract serves the
[local web interface](local-web-server.md); it is not a general JSON export API.

## Routes

| Route | Purpose |
| --- | --- |
| `GET /` | Embedded HTML |
| `GET /app.css`, `GET /app.js`, `GET /favicon.svg` | Embedded assets with appropriate MIME types |
| `POST /api/scan` | Validate one repository and stream scan events |
| `GET /api/recent` | List saved successful scans, newest first |
| `POST /api/recent/remove` | Remove one repository from the recent list |
| `GET /api/starred` | List starred skills, newest first |
| `POST /api/starred/add` | Star one skill |
| `POST /api/starred/remove` | Unstar one skill |

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

## Starred skills

`GET /api/starred` returns `{"enabled":true,"skills":[...]}`. Each entry
contains `repository` (normalized string), `path`, `name`, `commit` (full SHA
when starred), `link` (GitHub source URL pinned to that commit), and
`starred_at` (UTC milliseconds since the Unix epoch). Disabled starring returns
`enabled: false` and an empty list.

`POST /api/starred/add` takes
`{"repository":"example/skills","path":"skills/review/SKILL.md","name":"review","commit":"<full-sha>"}`.
`POST /api/starred/remove` takes `{"repository":"example/skills","path":"skills/review/SKILL.md"}`.
Both use the scan request's action header, content type, 4 KiB limit, unknown
field rejection, and Host/Origin checks. They return HTTP 204 on success,
including unstarring an absent skill, and HTTP 400 for an invalid repository,
path, commit, or name under the [starring rules](starred-skills.md#starring-and-persistence).
Starring while disabled returns HTTP 409; unstarring while disabled succeeds.
Storage failures return HTTP 500 with an actionable JSON `message`. These
routes do not contact GitHub, start a scan, or acquire its semaphore.

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
  finishes its scan.
