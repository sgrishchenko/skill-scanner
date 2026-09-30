"""Package a built native executable using only Python's standard library."""

import argparse
import hashlib
from pathlib import Path
import tarfile
import tomllib
import zipfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True)
    parser.add_argument("--tag")
    args = parser.parse_args()
    targets = {
        "x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu",
        "x86_64-apple-darwin", "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc",
    }
    if args.target not in targets:
        parser.error("unsupported release target")
    root = Path(__file__).resolve().parent.parent
    version = tomllib.loads((root / "Cargo.toml").read_text())["package"]["version"]
    if args.tag is not None and args.tag != f"v{version}":
        parser.error(f"tag must match Cargo.toml: v{version}")
    windows = "windows" in args.target
    executable = "skill-scanner.exe" if windows else "skill-scanner"
    binary = root / "target" / args.target / "release" / executable
    if not binary.is_file():
        parser.error(f"build the release executable first: {binary}")
    name = f"skill-scanner-v{version}-{args.target}"
    dist = root / "dist"
    dist.mkdir(exist_ok=True)
    files = [binary, root / "README.md", root / "LICENSE"]
    archive = dist / (name + (".zip" if windows else ".tar.gz"))
    if windows:
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as output:
            for source in files:
                output.write(source, f"{name}/{source.name}")
    else:
        with tarfile.open(archive, "w:gz") as output:
            for source in files:
                output.add(source, arcname=f"{name}/{source.name}")
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_name(archive.name + ".sha256").write_text(
        f"{digest}  {archive.name}\n", encoding="utf-8"
    )
    print(archive)


if __name__ == "__main__":
    main()
