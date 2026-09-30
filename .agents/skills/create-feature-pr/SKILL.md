---
name: create-feature-pr
description: Prepare and create a feature pull request for Skill Scanner using the repository's contribution guide and PR template, with inline demos from passing E2E tests and screenshot assertions for visual changes. Use when asked to open a feature PR or prepare its title and description.
---

# Create Feature PR

Turn the requested feature work into a focused, accurately described pull
request. For a request for PR text only, inspect the existing changes and
validation evidence and return the text without implementing or publishing
changes. A request to open a PR includes the necessary commit, push, and PR
creation.

## Read the maintained guidance

Locate the repository root and read these files before preparing the PR:

- [AGENTS.md](../../../AGENTS.md) for working constraints and verification.
- [CONTRIBUTING.md](../../../CONTRIBUTING.md) for PR scope, descriptions,
  validation, and readiness rules.
- [The PR template](../../../.github/pull_request_template.md) for the current
  description sections and checklist.
- [The specification index](../../../spec/README.md) and the feature specs
  affected by the change for requirements and acceptance checks.

Consult [implementation decisions](../../../spec/implementation.md) for
architecture and verification approaches, and [README.md](../../../README.md)
for setup, usage, and dependency compatibility notes when relevant. These
documents own the rules; read their current contents instead of maintaining
copies in this skill.

## Establish the PR scope

Inspect the branch, remotes, working tree, staged changes, and feature commits.
Determine the intended base branch and destination from the user's request and
repository configuration; do not assume `main` or `origin`. Review the complete
diff against that base, including pending changes intended for the PR.

Identify the feature's purpose, resulting behavior, affected specs, and any
remaining work. Preserve unrelated work and keep it out of the PR. If unrelated
commits or edits cannot be separated safely, clarify their ownership before
changing history or including them. Check for an existing PR for the same head
and base before creating another one.

## Prepare and verify the feature

Complete work already authorized by the user and update tests, feature specs,
and other documentation as required by the contribution guide. If feature work
remains outside the requested scope, describe it and keep the PR in draft.

Run the applicable validation from `CONTRIBUTING.md` at the repository root:

- Code changes require the documented formatting, Clippy, and locked Cargo
  test checks.
- Browser behavior changes also require rebuilding embedded assets and checking
  the relevant acceptance scenarios with mocked scan responses.
- Documentation-only changes require link and specification consistency checks;
  local Rust checks are unnecessary.

Use local fixtures and mocked HTTP responses for verification. Record actual
commands or scenarios and their outcomes, including skipped or blocked checks.
Reuse recorded results only when they still apply to the final change; rerun
the applicable checks after further code changes. Work, required validation, or
a required video demonstration that remains incomplete requires a draft PR.

## Record visual feature demonstrations

For features that change visible appearance or user interactions, follow the
[video demonstration requirements](../../../CONTRIBUTING.md#visual-feature-demonstrations).
Every included video demo, even an optional demo in a nonvisual PR or an animated
GIF version, must come from a passing E2E test with screenshot assertions at key
steps.

1. Map the demonstrated user flow to the relevant acceptance checks. Reuse or
   extend the existing E2E runner; if none exists, add the runnable test setup
   needed to reproduce the demo. Keep the test, fixtures, and screenshot
   baselines in the repository, and document the actual setup and run commands.
2. Drive the rebuilt application through that flow using local fixtures,
   mocked responses, and isolated storage. Add behavioral assertions and named
   screenshot comparisons at the starting state, key transitions, and final
   outcome, plus error or recovery states demonstrated by the video. For
   Playwright Test, use `expect(page).toHaveScreenshot('checkpoint.png')` or an
   equivalent assertion that fails on visual differences; `page.screenshot()`
   alone only captures an image.
3. Stabilize visual inputs and review any new or changed baselines. Enable video
   capture for a run with baseline updates disabled, and keep the recording
   from the same run only after all behavioral and screenshot assertions pass.
   A separate capture script, manual walkthrough, or failed run is insufficient.
4. Alongside the recording in the PR, include test and baseline or report links,
   the exact reproduction command, tested revision, checkpoint names, and result.

Embed the recorded demo in the PR description's Video demonstration section so
reviewers can watch it inline without downloading it or leaving the PR. Use a
GitHub video attachment that renders as a player. If attachment upload is
unavailable, embed an animated preview of the same recording and link the full
video, following the contribution guide. A download link, repository file link,
local path, or static screenshots alone do not satisfy the requirement.

Verify the rendered PR contains a playable video or animated preview accessible
to reviewers, and check access to the test evidence. After relevant code,
fixture, or baseline changes, rerun the test and replace the recording. If test
validation, recording, sharing, or inline playback is blocked, explain the
blocker and keep the PR in draft until the tested inline demo is available.
For nonvisual features with no optional demo, mark the section as not applicable.
For PR-text-only requests, use existing video evidence and identify missing
evidence without recording or uploading anything.

## Write the title and description

Start with the current PR template and preserve its sections and checklist.
Replace its instructional comments with concrete content. Write a short title
describing the final change. Explain the problem and resulting behavior, linking
relevant specs and an issue when one exists; an issue is optional.

Include validation evidence, the required inline demonstration for visual
features, and material risks or limitations. Screenshots may supplement the
video when useful. Mark checklist items only when supported by the actual work.
Keep the title and description aligned with the final diff, including when
updating an existing PR.

## Publish and report

When opening a PR is authorized, commit only the reviewed feature changes on an
appropriate feature branch and push to the intended remote. Preserve existing
commits and avoid force-pushing or rewriting unrelated history. Use the
available GitHub integration or `gh` to create the PR with an explicit
repository, base, head, title, and description; update a matching existing PR
instead of duplicating it. Set draft status when required by the guide.

With `gh`, write the description to a temporary Markdown file and pass it with
`--body-file` so newlines and literal text are preserved. If a creation request
fails ambiguously, check whether the PR exists before retrying. If publication
is blocked by authentication, permissions, or connectivity, keep the prepared
title and body and report the blocker without claiming the PR was created.

Confirm the resulting PR's URL, branches, title, description, and draft status.
For visual features, confirm the demo renders inline in the saved description.
Report the URL, readiness, validation results, and any remaining work. For a
text-only request, return the prepared title and body. Creating the PR does not
authorize merging it; leave it open for review under the contribution guide.
