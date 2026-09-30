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
  when applicable. Include screenshots for visual changes when they help review.
- Keep the title and description aligned with the final implementation as the
  scope changes. Use a draft PR while work or required validation is incomplete.

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

## Review and merge

- Request review once the change and its required validation are ready.
- Address review feedback and resolve outstanding concerns before merging.
- Merge only after the PR is out of draft and all existing CI jobs pass
  for the latest revision. The [CI workflow](.github/workflows/ci.yml) checks
  formatting, Clippy, Rust 1.84 compatibility, and builds and tests on all six
  supported platform targets.
- When code changes after validation, rerun the applicable checks and update
  the PR's validation results.
