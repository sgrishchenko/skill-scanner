# Skill Scanner

A Rust command-line application that discovers AI skills in **one public GitHub
repository**. A skill is any directory containing a regular file named exactly
`SKILL.md`, including the repository root and hidden directories.

```console
skill-scanner scan OWNER/REPO
skill-scanner scan https://github.com/OWNER/REPO
skill-scanner --help
skill-scanner --version
```

The scanner resolves the default branch to one commit, discovers skills, reads
YAML `name` and `description` fields, and prints links to their source at that
commit. It never runs repository scripts or skill instructions, installs skills,
or follows symlinks or submodules. It uses GitHub's API without requiring Git.

## Install

From this source directory, with [Rust 1.84 or newer](https://rustup.rs/):

```sh
cargo install --path . --locked
```

Cargo installs the executable in its bin directory (`~/.cargo/bin` on Linux and
macOS, `%USERPROFILE%\.cargo\bin` on Windows). Add that directory to `PATH` if
needed. To run directly from source:

```sh
cargo run --locked -- scan OWNER/REPO
```

Once a release has been published, download its archive for your OS and CPU,
verify its SHA-256 checksum, extract it, and move `skill-scanner` (or
`skill-scanner.exe`) to a directory on `PATH`. These binaries need no Rust
installation. No prebuilt release is included in this initial source tree.

| OS | x86-64 target | ARM64 target |
| --- | --- | --- |
| Linux (glibc 2.39+) | `x86_64-unknown-linux-gnu` | `aarch64-unknown-linux-gnu` |
| macOS | `x86_64-apple-darwin` | `aarch64-apple-darwin` |
| Windows | `x86_64-pc-windows-msvc` | `aarch64-pc-windows-msvc` |

The workflows build and test natively on Ubuntu 24.04, macOS 15, Windows Server
2022 (x86-64), and Windows 11 (ARM64). Source builds can target older operating
systems supported by their installed Rust toolchain. Release binaries are not
code-signed or notarized.

## Output

An illustrative scan looks like this (the commit is shortened here only):

```text
Repository: example/agent-tools
Commit: 0123456789abcdef...
Skills found: 2

code-review
  Description: Review changes for correctness and maintainability.
  Path: skills/code-review/SKILL.md
  Link: https://github.com/example/agent-tools/blob/0123456789abcdef.../skills/code-review/SKILL.md

release-notes
  Description: Draft release notes from a set of changes.
  Path: skills/release-notes/SKILL.md
  Link: https://github.com/example/agent-tools/blob/0123456789abcdef.../skills/release-notes/SKILL.md
```

Results are sorted by path; duplicate names remain separate. Missing or malformed
metadata produces a warning identifying the path and field on **stderr**, while
the discovered file remains in the report on **stdout**. Names fall back to the
containing directory, or the repository name for a root-level file. Descriptions
are empty when unavailable. Discovery does not certify a skill's validity,
compatibility, or safety.

The report uses plain text and works when redirected:

```sh
skill-scanner scan OWNER/REPO > skills.txt 2> warnings.txt
```

A completed scan with no matches prints `Skills found: 0` and
`No SKILL.md files found.` Empty repositories additionally report that there is
no commit to scan. A failed scan produces no inventory.

| Exit code | Meaning |
| --- | --- |
| `0` | Completed scan (including zero results or metadata warnings), help, or version |
| `1` | Scan or report could not be completed |
| `2` | Invalid command usage or repository input |

## GitHub access and limits

Public repositories work without credentials, subject to GitHub's anonymous API
rate limit (typically 60 requests/hour per IP). Set `GITHUB_TOKEN` in the process
environment to use higher public API limits. Private repositories remain
unsupported even if the token grants access. Tokens are never printed. If a
token is rejected, unset it for anonymous access or provide a valid token.

Each scan requests repository details, the default-branch commit, its tree, and
each eligible skill blob. Truncated recursive trees trigger a complete walk of
individual directory trees. An incomplete listing or failed download makes the
scan fail; partial results are never reported as a completed inventory.

Connections time out after 10 seconds, with a 30-second total timeout per
request. Transient failures are retried up to twice (250 ms, then 500 ms backoff).
Short `Retry-After` delays are honored; longer delays and access/rate-limit
failures produce actionable errors. Skill files over 1 MiB remain listed with
metadata warnings; API responses over 32 MiB fail the scan.

The input accepts `owner/repo` or an HTTPS `github.com` repository URL with an
optional `.git` suffix or trailing slash. Branch/file URLs, local repositories,
multiple repositories, installation, execution, and JSON export are outside
this version's scope.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
```

Tests use local HTTP mocks and repository fixtures, without real GitHub access
or tokens. Keep `Cargo.lock` checked in; it includes a `yoke-derive` version
compatible with Rust 1.84 (0.8.3 incorrectly uses a newer standard-library API).

Modules separate argument parsing, repository input, GitHub transport, discovery,
metadata, and rendering. See the [product specification](spec/cli.md) and
[implementation decisions](spec/implementation.md).

## Releasing

Update `Cargo.toml` and `Cargo.lock`, then push a matching `vVERSION` tag. The
release workflow tests and builds all six targets, packages the executable with
this README and the MIT license, and creates a **draft** GitHub release with
archives and SHA-256 checksums. Review and publish the draft when ready.

To package a local target manually (Python 3.11+):

```sh
cargo build --release --locked --target x86_64-unknown-linux-gnu
python3 scripts/package.py --target x86_64-unknown-linux-gnu
```

Artifacts are written to `dist/`. The script accepts `--tag vVERSION` to verify
that a release tag matches the crate version.
