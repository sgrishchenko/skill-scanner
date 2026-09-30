## Problem and change

<!-- Explain the problem and resulting behavior. Link relevant specs and an
issue if one exists. Keep this description aligned with the final change. -->

## Validation

<!-- List the commands or acceptance checks run and their results.
Code: cargo fmt --all -- --check; cargo clippy --locked --all-targets -- -D warnings;
cargo test --locked.
Browser behavior: rebuild and check the relevant scenarios with mocked scan responses.
Documentation only: check links and consistency with the specs; local Rust checks are unnecessary.
For every PR, wait for all CI checks on the latest revision, including the full
matrix and checks not required by branch protection. Record the verified commit
and CI run links/results here. Diagnose failures, commit fixes to this PR, and
verify all checks again after pushing. Pending or unsuccessful checks are not
passing; keep this item unchecked until all checks pass. Explain skipped or
blocked validation and keep externally blocked PRs in draft.
See CONTRIBUTING.md#ci-completion. -->

## Video demonstration

<!-- Required for features that change visible appearance or user interactions:
embed the recorded demo here so reviewers can watch it inline without downloading
it or leaving the PR. Use a GitHub video attachment; if upload is unavailable,
embed an animated preview of the same recording and link the full video.
Show the relevant user actions and visible result from the final implementation
using local fixtures and mocked responses. For browser features, rebuild the
executable before recording to embed the updated assets.
Every video/GIF demo, including optional demos, must come from the same passing
E2E test run that checks behavior and compares screenshots against reviewed
baselines at key steps. Link the test and screenshot baselines or report, and
include the exact reproduction command, tested revision, named screenshot
checkpoints, and result. Review baseline changes and record with baseline
updates disabled. Rerun and replace the recording after relevant changes.
Verify inline playback and reviewer access in the rendered PR. Download links,
repository file links, local paths, or static screenshots alone are insufficient.
If test validation, recording, sharing, or inline playback is blocked, explain
why and keep the PR in draft until the tested inline demo is available.
Write "Not applicable" for nonvisual features with no optional demo.
See CONTRIBUTING.md#visual-feature-demonstrations. -->

## Risks or limitations

<!-- Describe material compatibility concerns, limitations, or follow-up work.
Write "None" if there are none. -->

## Checklist

- [ ] This PR has one purpose and a title that describes the change.
- [ ] Relevant specs and documentation are updated, or no updates are needed.
- [ ] Tests cover the changed behavior where useful, or no test changes are needed.
- [ ] Validation results above are accurate, including any skipped or blocked checks.
- [ ] All CI checks pass on the latest PR revision, and any fixes are included
  in this PR; the verified commit and CI results are recorded above.
- [ ] The final visual feature's demo is watchable inline above and accessible
  to reviewers, or this PR has no visual or interaction changes.
- [ ] Every included video demo comes from a passing E2E test with screenshot
  assertions at key steps and reproduction details above, or no video is included.

<!-- See CONTRIBUTING.md for the full PR rules. Before merging, address review
feedback and wait for all CI jobs to pass on the latest revision. -->
