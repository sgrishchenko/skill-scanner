# Web results

Status: implemented. Presents the completed inventory from a
[web scan](web-scan.md), using shared [metadata](skill-metadata.md) and
[similarity grouping](similarity-grouping.md) rules.

## Summary and skill cards

Display the repository and scanned commit. Shorten the commit visually while
including the full commit in the commit link's accessible label/title. Show
total skills, skills with metadata warnings, similar-group count, grouped
skills and their percentage of all skills, standalone skills, and largest
similar-group size (zero when there are no similar groups). Statistics cover
the entire scan and remain visible for successful empty scans with zero values.

Each card shows the skill's name, description, repository-relative path, and
commit-pinned GitHub source link. Use the scanner's directory/repository name
fallback. Missing descriptions say “No description available.” Let users expand
metadata warnings to read field-specific messages. Every source file remains
individually accessible.

Repository text and links follow the
[browser content protection](local-web-server.md#browser-content-protection)
rules.

## Grouped and all-skills views

Default to **Grouped skills**. Show similar groups largest first, breaking ties
by their first member path. Each group shows its size, percentage of all skills,
and skills with warnings. Expand a group to inspect member cards sorted by path.
Standalone skills appear as individual cards.

**All skills** displays every card sorted by path. Explain the shared
[metadata matching rules](similarity-grouping.md#matching-rules) in the interface.

## Search and filters

Search names, descriptions, and paths by case-insensitive substring. Optionally
show only similar skills or skills with warnings. Filters combine with AND,
apply to individual members in both views, and run locally without additional
GitHub requests.

Open matching groups automatically when searching or filtering by warnings,
and state how many members remain visible. A group remains similar even when
filters leave only one visible member. Summary and group statistics always
describe the complete scan; filtering changes only visible members.

Show visible/total skill and group counts. When no cards match, retain the
inventory and show a clear-filters action. This state is distinct from a
completed scan with no skills, an empty repository, and a failed scan.
Reset filters and the view on each new scan.

## Organization results

For an [organization scan](organization-scan.md#inventory-and-aggregation), the
summary shows the organization, the number of public repositories scanned,
repositories with skills, and skipped forks. A **Repositories** section lists
failed repositories first with their diagnostics, then repositories with skills,
their skill counts, and shortened commit links with full-commit labels.
Repositories without skills are collapsed into an expandable list that marks
empty repositories. Any failed repository shows an incomplete-scan notice and an
incomplete results status; its skills are missing rather than counted as zero.

Skill cards, statistics, grouped and all-skills views, and filters behave as for
a repository scan across all scanned repositories. Card paths include the
repository, so searching for `owner/repository/` shows one repository's skills.
An organization with no skills says no `SKILL.md` files were found; one with no
repositories to scan, or none that could be scanned, says so instead.

## Accessibility and layout

Use labeled controls, semantic headings, keyboard operation, visible focus, a
skip link, live progress/error announcements, and reduced-motion styles.
Adapt to narrow screens without horizontal scrolling. Desktop and narrow-screen
layouts support the same flow.

## Acceptance checks

- A successful scan renders pinned links, duplicate names, visible metadata
  warnings, and fallback text without hiding source files.
- Grouped/all views retain every source. Similarity statistics and group counts
  agree with the CLI and remain stable when filters reduce visible members.
- Search, similar-only, and warning filters combine correctly in both views
  and can be cleared. Missing metadata does not create false similarity groups.
- Filtering all cards away preserves the inventory and offers a clear-filters
  action; empty scans retain their summary statistics.
- Desktop and narrow-screen layouts support the same keyboard-accessible flow.
- Organization results list failures, repositories with skills, and
  repositories without skills, and group matching skills across repositories.
