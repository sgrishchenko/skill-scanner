# Codex skills

Status: implemented. People install a discovered skill for the Codex agent
from [web results](web-results.md) or the CLI, and remove it again. Installing
copies the skill's files into the directory where Codex discovers personal
skills; it never runs the skill, its scripts, or its instructions, and it is not
a quality or safety rating. Review a skill's source before installing it.

## Installation directory

Codex discovers personal skills in `$HOME/.agents/skills/<name>/SKILL.md`.

| Platform | Default Codex skills directory |
| --- | --- |
| Linux, macOS, and other Unix systems | `$HOME/.agents/skills` |
| Windows | `%USERPROFILE%\.agents\skills` |

`SKILL_SCANNER_CODEX_SKILLS_DIR` overrides the entire directory. Relative
values resolve against the process working directory; an empty value disables
installation. If the home directory is unset or empty, installation is
disabled. Create the directory only when installing a skill. The legacy
`$CODEX_HOME/skills` directory and repository-level `.agents/skills`
directories are not used.

## Folder names and ownership

- A skill's folder is named after the directory containing its `SKILL.md`, or
  the repository name for a root-level `SKILL.md`. Folder names start with an
  ASCII letter or digit, use only ASCII letters, digits, `.`, `_`, and `-`, do
  not end with `.`, are at most 100 characters, and are not Windows device names
  such as `CON` or `nul.txt`. Other skills cannot be installed.
- Each installed folder contains a `.skill-scanner.json` record with a format
  version, the normalized repository, the repository-relative `SKILL.md` path,
  the full commit, and the install time (UTC milliseconds since the Unix epoch).
  Only folders with a valid record whose path produces that folder's name are
  installed by Skill Scanner. Hidden folders, symlinks, and malformed,
  incompatible, mismatched, or oversized (over 16 KiB) records are ignored.
- Folders without such a record are never listed, replaced, or removed.
  Installing over one is a conflict. Installing a skill whose folder holds a
  different repository or path is also a conflict that names the installed
  skill; remove it first. Repository input forms and case variants address the
  same installation; paths are case-sensitive.
- Installing the same skill again replaces its folder with the newly
  downloaded files, discarding local edits.

## Downloading and publishing

- Accept only the repository-relative `SKILL.md` paths a scan can report, as in
  [starred skills](starred-skills.md#starring-and-persistence). Check the folder
  for conflicts before contacting GitHub.
- Use the same GitHub client, credentials, limits, and errors as scanning, as
  defined by [repository access](repository-access.md). Private repositories are
  rejected. The web interface installs the full commit the results were scanned
  at; the CLI installs the default branch's current commit. A missing directory
  or a commit GitHub does not confirm fails.
- Walk the pinned tree to the skill's directory and list it recursively. A
  truncated listing fails. Copy every regular file except symlinks,
  submodules, a `.skill-scanner.json` at the top, and nested skills: any
  subdirectory with its own regular `SKILL.md` is left out with its contents.
  The skill's own `SKILL.md` must be a regular file.
- A skill is limited to 1000 files and 16 MiB in total. Every file name must be
  portable: no `\`, `:`, `*`, `?`, `"`, `<`, `>`, `|`, or control characters,
  no trailing `.` or space, no Windows device names, and no two paths that
  differ only in case. Otherwise the installation fails without writing files.
- Download every file before writing anything. Write into a hidden sibling
  folder, then rename it into place; a replaced installation is restored if the
  final rename fails. Failures leave the previous state and no partial folder
  visible to Codex. Executable repository files stay executable on Unix.
- Codex detects new skills automatically; restart it if one does not appear.

## Removal

Removing a skill by folder name deletes its whole folder, including any local
edits. Removing an absent folder succeeds and reports that nothing was
removed. A folder not installed by Skill Scanner is a conflict and is left
unchanged. If deletion fails, the folder is restored where possible and the
error reported.

## Web interface

Each skill card has an **Install for Codex** button labeled with its path. For
an installed skill from the same repository and path, it reads
**Remove from Codex** and removes the folder. While a request is pending, the
button reads **Installing…** or **Removing…**, keeps keyboard focus, and
ignores repeated presses. Organization results install each skill from its own
repository, repository-relative path, and repository commit.

The scan form shows **Codex skills**: the installation directory, a status line,
and every skill installed by Skill Scanner, sorted by case-insensitive folder
name. Each entry shows the folder name, repository, and path, a commit-pinned
**Source** link that follows the
[browser content protection](local-web-server.md#browser-content-protection)
rules, and a **Remove** button. The list loads on page open, refreshes after
each change and on window focus, and shows empty, disabled, and unavailable
states. Pending and completed changes are announced in its status line, and
failures such as conflicts appear in its alert without changing the list.
Removing from the list moves focus to the list heading. When installation is
disabled, card buttons are hidden.

The server runs one Codex installation or removal at a time, independent of
the one-scan-per-server rule; a concurrent change is rejected with a message.
Installing does not start a scan. Closing the page lets a started installation
finish.

## CLI

```text
skill-scanner codex
skill-scanner codex install OWNER/REPO PATH
skill-scanner codex remove NAME
```

`skill-scanner codex` lists installed skills with each folder name,
repository, path, and commit-pinned link, without contacting GitHub, and
explains empty and disabled lists. `codex install` accepts the same repository
input forms as scanning, prints progress before each request to stderr, and
prints the installed folder, file count, and commit to stdout. `codex remove`
removes one folder without contacting GitHub. Invalid repositories, paths, and
folder names exit 2; conflicts, disabled installation, GitHub failures, and
storage errors exit 1 with a diagnostic. Commands follow the CLI's stream,
escaping, and broken-pipe conventions.

## Acceptance checks

Use temporary directories and mocked GitHub or browser responses only. The
[reproducible browser demo test](../../tests/e2e/README.md) covers installing
from a card with its pending state, a folder-name conflict reported by the
server, keyboard installation, persistence after a restart, and removal from
the list and from a card, with named screenshot assertions.

- Installed folders contain the skill's regular files, preserve executable
  modes on Unix, and exclude nested skills, symlinks, submodules, and copied
  records. Root-level skills use the repository name.
- A new library instance, CLI process, or web server sees prior installations.
  Reinstalling replaces the folder; removal deletes it and persists.
- Folders not installed by Skill Scanner, or installed from another skill, are
  never replaced or removed, and conflicts make no GitHub requests.
- Invalid paths, folder names, commits, and repositories, unportable or
  case-colliding file names, oversized or truncated skills, missing
  directories, unconfirmed commits, private or empty repositories, and failed
  downloads write no files and keep any previous installation.
- Disabled installation creates no files; a regular file used as the directory
  is preserved and causes storage errors. Scans succeed regardless.
- Web endpoints enforce the
  [local request protections](local-web-server.md#local-request-protection),
  report GitHub failures without server error text, and allow one change at a
  time while scans keep their own limit.
