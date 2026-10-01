# Organization scan demonstration

![Organization scan demonstration](organization-scan.gif)

[Watch the MP4 version](organization-scan.mp4).

This recording comes from the passing
[organization scan E2E test](../../tests/e2e/organization-scan.spec.mjs),
with baseline updates disabled. The [recording manifest](organization-scan.json)
identifies the tested source revision, pinned browser environment, screenshot
checkpoints, pass result, and media hashes. A later media-only commit adds the
recording without changing the tested source or baselines.

The test drives the rebuilt executable using
[organization scan fixtures](../../tests/e2e/fixtures/organization-scans.json),
isolated history, starred, and cache directories, and no credentials or live
GitHub access. Only `/api/scan-org` responses are mocked; recent repositories,
starring, and CLI reads use the actual application and persistent storage.

Five [reviewed screenshot baselines](../../tests/e2e/snapshots/organization-scan.spec.mjs)
cover switching to organization mode, the pending scan, combined results with a
failed repository and a similar group spanning repositories, starring a skill and
filtering by repository, and a rate limit that stops the scan. Behavioral
assertions run before each comparison, including disabled controls while
scanning, the starred entry's repository, path, and pinned commit in the list and
CLI, and the unchanged recent repositories list.

Follow the [setup and exact reproduction commands](../../tests/e2e/README.md)
to run the test and export its video. The MP4 and inline GIF are conversions of
the same passing run, with the tested sequence and outcome preserved. CI runs
the same test and retains its report, video, and failure diagnostics.
