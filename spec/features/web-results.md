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
commit-pinned GitHub source link, plus a **Star** toggle defined by
[starred skills](starred-skills.md#web-interface). Use the scanner's directory/repository name
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
show only similar skills, skills with warnings, or
[starred skills](starred-skills.md#web-interface). Filters combine with AND,
apply to individual members in both views, and run locally without additional
GitHub requests.

Open matching groups automatically when searching or filtering by warnings or
stars,
and state how many members remain visible. A group remains similar even when
filters leave only one visible member. Summary and group statistics always
describe the complete scan; filtering changes only visible members.

Show visible/total skill and group counts. When no cards match, retain the
inventory and show a clear-filters action. This state is distinct from a
completed scan with no skills, an empty repository, and a failed scan.
Reset filters and the view on each new scan.

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
- Search, similar-only, warning, and starred-only filters combine correctly in
  both views and can be cleared. Missing metadata does not create false similarity groups.
- Filtering all cards away preserves the inventory and offers a clear-filters
  action; empty scans retain their summary statistics.
- Desktop and narrow-screen layouts support the same keyboard-accessible flow.
