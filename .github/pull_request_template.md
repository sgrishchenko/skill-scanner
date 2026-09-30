## Problem and change

<!-- Explain the problem and resulting behavior. Link relevant specs and an
issue if one exists. Keep this description aligned with the final change. -->

## Validation

<!-- List the commands or acceptance checks run and their results.
Code: cargo fmt --all -- --check; cargo clippy --locked --all-targets -- -D warnings;
cargo test --locked.
Browser behavior: rebuild and check the relevant scenarios with mocked scan responses.
Documentation only: check links and consistency with the specs; local Rust checks are unnecessary.
Explain any skipped or blocked checks. -->

## Video demonstration

<!-- Required for features that change visible appearance or user interactions:
attach a recorded video or link to one reviewers can access. Show the relevant
user actions and visible result from the final implementation using local
fixtures and mocked responses. For browser features, rebuild the executable
before recording to embed the updated assets.
Screenshots may supplement the video but do not replace it; a local path is insufficient.
If recording or sharing is blocked, explain why and keep the PR in draft until
the video is provided. Otherwise, write "Not applicable" for nonvisual features. -->

## Risks or limitations

<!-- Describe material compatibility concerns, limitations, or follow-up work.
Write "None" if there are none. -->

## Checklist

- [ ] This PR has one purpose and a title that describes the change.
- [ ] Relevant specs and documentation are updated, or no updates are needed.
- [ ] Tests cover the changed behavior where useful, or no test changes are needed.
- [ ] Validation results above are accurate, including any skipped or blocked checks.
- [ ] A video demonstration of the final visual feature is provided above, or
  this PR has no visual or interaction changes.

<!-- See CONTRIBUTING.md for the full PR rules. Before merging, address review
feedback and wait for all CI jobs to pass on the latest revision. -->
