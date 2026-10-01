# Browser E2E tests and demo recording

Each test drives the rebuilt application and records the same run that checks
behavior and compares named screenshots against reviewed baselines:

- The [recent repositories test](recent-repositories.spec.mjs) has nine
  [baselines](snapshots/recent-repositories.spec.mjs) and a recorded
  [demo](../../docs/demos/recent-repositories.md).
- The [organization scan test](organization-scan.spec.mjs) has five
  [baselines](snapshots/organization-scan.spec.mjs).

Demos follow the
[PR demonstration requirements](../../CONTRIBUTING.md#visual-feature-demonstrations).

## Reproduce

Use Rust 1.84+, Node.js 20+, npm, and Docker on an x86-64 Linux host. Node.js and
the browser tooling are development dependencies only; the application still
requires no frontend build. The canonical screenshot environment is the pinned
Playwright Ubuntu 24.04 container, also used by the `browser-e2e` CI job.

From the repository root:

```sh
cargo build --locked
npm ci --prefix tests/e2e
docker run --rm --init --ipc=host --platform linux/amd64 \
  --user "$(id -u):$(id -g)" \
  --volume "$PWD:/work" --workdir /work \
  mcr.microsoft.com/playwright:v1.63.0-noble npm test --prefix tests/e2e
```

Always rebuild first, including after browser asset changes. The test starts
`target/debug/skill-scanner` on an available loopback port; the recent
repositories test restarts it twice on newly allocated ports. Each run gets history and cache directories under
its own Playwright output directory. Tokens are removed from child process
environments. External browser requests are blocked and fail the test; a dead
proxy also prevents accidental server-side GitHub access.

Only `/api/scan` responses and their successful-scan history writes are mocked
using [fixed fixtures](fixtures/scans.json), and `/api/scan-org` responses use
complete [organization event streams](fixtures/organization-scans.json) without
history writes. Each test treats the other scan route as an unexpected request.
The embedded browser assets,
`/api/recent`, `/api/recent/remove`, disk persistence, and `skill-scanner recent`
run unchanged. This tests the browser workflow and shared history, while
[Rust scan tests](../../src/recent_scan_tests.rs) independently verify actual
scanner recording behavior with mocked GitHub responses.

Viewport, device scale, browser version, locale, UTC timezone, motion preference,
and fixture timestamps are fixed. Chromium uses software rendering, full tile
rasterization, and the sRGB color profile. Screenshots cover the full page,
including the skill cards below the video viewport. No application content is
masked. Scan responses are held until disabled controls are asserted; DOM assertions determine
readiness before each screenshot. Short pauses after successful comparisons
make the recorded actions readable. Pixel differences fail the test, retries
are disabled, and `npm test` never creates or updates a baseline.

The HTML report is in `tests/e2e/playwright-report/`; JSON, video, and failure
traces/diffs are in `tests/e2e/test-results/`. CI uploads both directories as the
`browser-e2e` artifact, including on failure. They are ignored by Git.

## Review baseline changes

Use the same build, install, and Docker commands, replacing `npm test` with
`npm run snapshots`. Open and review every changed PNG for correctness. Commit
the test, fixtures, and reviewed baselines, then rerun the normal command with
updates disabled. Keep the Playwright package lock, image tag, and documented
environment in sync when upgrading the browser.

The recent repositories checkpoints are:

| Screenshot | Behavior asserted before comparison |
| --- | --- |
| `01-empty-history` | Empty recent list and initial results state |
| `02-scanning` | Pending scan disables submission/input and hides results |
| `03-first-scan` | Two skills and the first saved repository with a fixed timestamp |
| `04-newest-first` | Second repository sorts first with correct counts/times |
| `05-restart-persistence` | Restart preserves history and CLI order; results reset |
| `06-select-saved-repository` | Keyboard selection fills/focuses the field without scanning |
| `07-repeat-scan-updates-entry` | Repeat scan updates count/time and order without duplication |
| `08-removal-keeps-current-results` | Removing the currently displayed repository retains all its results |
| `09-removal-persists-after-restart` | Another restart and CLI confirm persistent removal |

The test also checks disabled recent controls during scans, the exact scan
request sequence, absence of external requests, and absence of JavaScript errors.

The organization scan checkpoints are:

| Screenshot | Behavior asserted before comparison |
| --- | --- |
| `01-organization-mode` | Toggle switches the heading, label, placeholder, hint, and examples; an example fills and focuses the field without scanning |
| `02-scanning-organization` | Pending scan disables the field, mode toggle, examples, and submission and hides results |
| `03-organization-results` | Combined results list the failed repository first, repositories with skills and pinned commits, and a cross-repository similar group; recent repositories stay empty |
| `04-filter-by-repository` | Repositories without skills expand, and searching a repository path filters members while statistics stay unchanged |
| `05-rate-limit-stops-scan` | A rate limit shows the stopping diagnostic, restores the form, and shows no partial results |

It also checks the request bodies, a return to repository mode, absence of
external requests, and absence of JavaScript errors.
Error/recovery and mobile scenarios are outside this demo's sequence; their
broader acceptance requirements remain in the feature spec.

## Export the passing recording

After committing the test, fixtures, and reviewed baselines, run the normal
test command for only the demonstrated spec from a clean checkout, appending
`-- organization-scan.spec.mjs` (or another spec file) to `npm test`. With
FFmpeg installed on the host, export that spec's recording:

```sh
npm run demo --prefix tests/e2e -- organization-scan
```

Without an argument, the exporter uses `recent-repositories`. It requires a
clean tested commit, exactly one passing test from that spec, every screenshot
checkpoint in its baseline directory, no retries/skips, and baseline updates
disabled. It converts that run's video attachment into the checked-in
`docs/demos/<spec>.mp4` and animated `.gif`, without changing the sequence, and
writes a `.json` manifest with the tested revision,
environment, checkpoint names, and media hashes. It accepts Docker report paths.
Do not export a baseline-generation run or a separate recording script.

Commit the regenerated media and manifest, link this test and its baselines in
the PR, and embed the GIF with a link to the MP4 if GitHub video attachment upload
is unavailable. The manifest identifies the source commit used for recording;
the subsequent media-only commit does not change the tested code or baselines.
Rerun and replace the recording after any relevant source, fixture, or baseline
change. CI must still pass on the final PR commit.
