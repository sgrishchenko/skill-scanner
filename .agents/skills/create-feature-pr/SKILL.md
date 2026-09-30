---
name: create-feature-pr
description: Prepare and create a feature pull request for Skill Scanner using the repository's contribution guide and PR template, with demos watchable inline in the PR description for visual changes. Use when asked to open a feature PR or prepare its title and description.
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
Record the feature in the running application, showing the relevant user actions
and visible result from the final implementation. For browser features, rebuild
the executable before recording to embed updated assets. Use local fixtures and
mocked responses.

Embed the recorded demo in the PR description's Video demonstration section so
reviewers can watch it inline without downloading it or leaving the PR. Use a
GitHub video attachment that renders as a player. If attachment upload is
unavailable, embed an animated preview of the same recording and link the full
video, following the contribution guide. A download link, repository file link,
local path, or static screenshots alone do not satisfy the requirement.

Verify the rendered PR contains a playable video or animated preview accessible
to reviewers. Re-record if later changes make the demonstration inaccurate. If
recording, sharing, or inline playback is blocked, explain the blocker and keep
the PR in draft until the inline demo is available. For nonvisual features, mark
the section as not applicable. For PR-text-only requests, use existing video
evidence and identify missing evidence without recording or uploading anything.

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
