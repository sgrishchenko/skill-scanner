# Installation and releases

Status: implemented. Distributes the standalone `skill-scanner` executable for
the [CLI](cli-report.md) and [local web server](local-web-server.md).

## Source installation

Use Rust 1.84 or newer to build from source:

```sh
cargo install --path . --locked
```

Commit `Cargo.lock` and use `--locked` for builds and installation. Rust is
required only for source builds.

## Platforms and release packages

Support Linux (glibc), macOS, and Windows on x86-64 and ARM64. Native CI jobs
build and test each of these six targets. Linux release binaries require glibc
2.39 or newer; Cargo source builds can use the host's older glibc.

A `v*` tag triggers the release workflow, which tests and builds all six native
targets, packages the standalone executable with its license and README, and
publishes a draft GitHub release with SHA-256 checksums. Linux/macOS archives
use `.tar.gz`; Windows archives use `.zip`.

Users extract the archive and put the executable on `PATH`. The web interface's
assets are embedded, so installed binaries need no separate asset directory or
frontend runtime.

## Acceptance checks

- Source installation uses the committed lockfile and supported Rust toolchain.
- Native CI builds and tests all six operating-system/architecture targets.
- Release archives contain the executable, license, and README in the expected
  format, with SHA-256 checksums on a draft release.
- Installed binaries provide both interfaces without a Rust or frontend runtime.
