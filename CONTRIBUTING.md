# Contributing

Read the [specification index](spec/README.md) and the relevant feature specs
before changing behavior. Use [implementation decisions](spec/implementation.md)
for architecture and [the README](README.md) for setup and usage.

## Pull request scope and description

- Keep each PR focused on one purpose, including the tests and documentation
  needed to complete it. Split unrelated changes into separate PRs.
- Write a short, concrete title describing the change, such as
  `Fix cached scan progress in the web interface`.
- Use the [PR template](.github/pull_request_template.md). Explain the problem,
  the resulting behavior, and how you verified it. Link relevant specs and an
  issue when one exists; an issue is not required.
- Describe material compatibility concerns, limitations, or follow-up work
  when applicable.
- For features that change visible appearance or user interactions, include a
  recorded [video demonstration](#visual-feature-demonstrations) watchable inline
  in the PR description.
- Keep the title and description aligned with the final implementation as the
  scope changes. Use a draft PR while work, required validation, or a required
  video demonstration is incomplete.

## Project requirements

- Follow the [working constraints](AGENTS.md#working-constraints), including
  Rust 1.84 compatibility, edition 2021, the committed lockfile, shared scanning
  logic, and treating scanned content as untrusted data.
- Update the owning feature spec and its acceptance checks when behavior
  changes, the README when usage changes, and implementation decisions when
  architecture changes.
- Add or update tests where they meaningfully verify changed behavior or guard
  against a regression. Use local fixtures and mocked HTTP responses without
  live GitHub access or real credentials.

## Validation

After code changes, run from the repository root and record the results in
the PR:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

For browser behavior changes, rebuild the executable to embed the updated
assets and exercise the relevant feature acceptance checks with mocked scan
responses. Describe the scenarios checked and their results in the PR.

For documentation-only changes, check links and consistency with the existing
specs; local Rust checks are unnecessary. State which checks ran and explain
any skipped or blocked validation. Never report a check as passed unless it ran
successfully.

## Visual feature demonstrations

Every feature PR that changes visible appearance or user interactions must
include a recorded video demonstration of the feature in the running
application. Show the relevant user actions and visible result from the final
implementation.

Every video demo included in a PR, including an animated GIF version, must be
recorded from a passing, reproducible end-to-end (E2E) test. The test must drive
the actual application through the demonstrated user flow and assert its
behavior. A manual recording or a script that only captures screenshots does
not meet this requirement. This also applies to optional demos in nonvisual PRs.

- Keep the E2E test, fixtures, and screenshot baselines in the repository. Use
  local fixtures and mocked responses without live GitHub access or real
  credentials, and isolate persistent test storage. For browser demos, rebuild
  the executable first so it embeds the current assets.
- Add named screenshot assertions at the key steps: the starting state,
  meaningful intermediate states after user actions, and the final outcome.
  Include error or recovery states when they are part of the demonstrated
  flow. Assert the expected behavior before comparing each screenshot against
  a reviewed baseline; visual mismatches must fail the test. Saving screenshots
  or inspecting video frames alone is not a screenshot assertion.
- Keep visual checks deterministic with fixed fixtures, viewport, and browser
  settings. Wait for the expected UI state and stabilize changing timestamps
  or animations without masking the behavior under test. Review new or changed
  baselines for correctness, then rerun with baseline updates disabled.
- Record the same test execution that passes the behavioral and screenshot
  assertions. Do not substitute a separate demonstration script or publish a
  failed run as validation. Captions, trimming, and format conversion may
  improve readability, but must preserve the tested sequence and outcome.

Embed the recorded demo directly in the PR description's Video demonstration
section so reviewers can watch it without downloading it or leaving the PR.
Use a GitHub video attachment that renders as an inline player. If attachment
upload is unavailable, embed an animated preview of the same recording and link
the full video. A download link, repository file link, or local path alone is
insufficient. Static screenshots may supplement the demo but do not replace it.

Alongside the recording, link the E2E test and screenshot baselines or report.
Record the exact reproduction command, tested revision, key screenshot
checkpoints, and pass/fail result. Make the artifacts accessible to reviewers.

Verify the saved PR description renders a playable video or animated preview
and that the media is accessible to reviewers. After a relevant code, fixture,
or baseline change, rerun the test and replace the recording.

If test validation, recording, sharing, or inline playback is blocked, explain
the blocker in the PR and keep it in draft until the tested inline demo is
available. For features without visual or interaction changes and no optional
demo, mark the section as not applicable.

## Review and merge

- Request review once the change, its required validation, and any required
  video demonstration are ready.
- Address review feedback and resolve outstanding concerns before merging.
- Merge only after the PR is out of draft and all existing CI jobs pass
  for the latest revision. The [CI workflow](.github/workflows/ci.yml) checks
  formatting, Clippy, Rust 1.84 compatibility, and builds and tests on all six
  supported platform targets.
- When code changes after validation, rerun the applicable checks and update
  the PR's validation results.
