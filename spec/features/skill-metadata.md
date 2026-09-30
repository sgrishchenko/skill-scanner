# Skill metadata

Status: implemented. Applied to every [discovered skill](skill-discovery.md)
before either interface renders its inventory.

## Parsing and limits

Read at most 1 MiB per `SKILL.md`. Parse UTF-8 YAML front matter delimited by
`---` lines; a `...` closing delimiter is also accepted. Allow a UTF-8 BOM and
CRLF line endings.

Read `name` and `description` as nonempty strings. Missing, empty, malformed,
or wrongly typed fields produce warnings identifying the affected file and
field. Ignore other front-matter fields. Oversized, non-UTF-8, or
invalid-front-matter files remain in the inventory with metadata warnings.

## Fallbacks and warnings

Use the containing directory name as the display-name fallback, or the
repository name for a root-level `SKILL.md`. Leave unavailable descriptions
empty in the inventory. The [web results](web-results.md#summary-and-skill-cards)
display “No description available.” for those descriptions.

Warnings preserve discovered files and do not turn a completed scan into a
failure. The [CLI](cli-report.md#output-streams-and-progress) reports warnings
on stderr; web cards expose field-specific messages. Discovery alone does not
certify validity, compatibility, or safety.

[Similarity grouping](similarity-grouping.md) ignores fallback names,
unavailable fields, and normalized empty values when looking for matches.

## Acceptance checks

- Valid front matter supplies the displayed name and description; unrelated
  fields do not affect discovery.
- BOM, CRLF, and either supported closing delimiter are accepted.
- A file with missing or malformed metadata remains listed with a warning
  identifying its path and affected field.
- Oversized and non-UTF-8 files remain listed with metadata warnings.
- Missing names use the containing directory or root repository fallback;
  unavailable descriptions stay empty in the inventory.
- Metadata warnings are successful scan results in both interfaces.
