# Similarity grouping

Status: implemented. The CLI and web interface use the same aggregation result
for the [complete inventory](skill-discovery.md#scan-outcomes).

## Matching rules

- Group skills whose valid names or descriptions match after lowercasing and
  replacing punctuation/whitespace runs with a single space.
- Compare names only to names and descriptions only to descriptions.
- Ignore unavailable fields, fallback names, and normalized empty values.
- Merge connected matches so each skill belongs to exactly one group,
  including standalone skills. Two skills can belong to the same group through
  another member's matching metadata.
- A similar group contains at least two members. This is metadata similarity,
  not a claim of identical file contents or semantic equivalence.

## Ordering and statistics

Order groups by size descending, breaking ties by the first member path. Order
members by repository-relative path. Preserve every original skill and source
link.

Report total skills, similar-group count, skills in similar groups and their
percentage of all skills, standalone skills, largest similar-group size, and
skills with warnings. The largest similar group is zero when there are no
similar groups. Each group reports its member count, percentage of all skills,
and number of skills with warnings.

Percentages use the full scan's skill count as the denominator, with zero for
an empty scan. Empty inventories have no groups and all-zero statistics.
Count skills with warnings, not the number of warning messages.

The [CLI report](cli-report.md#terminal-inventory) and
[web results](web-results.md) define presentation. Browser filtering changes
visible members only: statistics always describe the full scan, and a group
remains similar even if only one member is visible. The
[web scan API](web-scan-api.md#inventory-and-aggregation) defines serialization.

## Acceptance checks

- Case, spacing, and punctuation differences in valid matching metadata do not
  prevent grouping.
- Connected matches form one group without duplicating skills.
- Missing metadata, fallback names, normalized empty values, and cross-field
  name/description matches do not create false similarity groups.
- CLI and web group counts and statistics agree, including empty inventories
  and inventories containing only standalone skills.
- Equal-size groups and their members follow the path-based ordering rules.
- Filtering visible members leaves similarity membership and statistics intact.
