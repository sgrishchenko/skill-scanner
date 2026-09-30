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
  recorded [video demonstration](#visual-feature-demonstrations) in the PR.
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
implementation. For browser features, rebuild the executable before recording
to include the updated assets. Use local fixtures and mocked responses without
live GitHub access or real credentials.

Attach the video or provide a link reviewers can access in the PR template's
Video demonstration section. A local file path is insufficient. Screenshots may
supplement the video but do not replace it. Re-record the demonstration if later
changes make it inaccurate.

If recording or sharing the video is blocked, explain the blocker in the PR and
keep it in draft until the video is provided. For features without visual or
interaction changes, mark the section as not applicable.

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
