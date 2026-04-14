#!/usr/bin/env python3

from __future__ import annotations

import argparse
import shutil
import subprocess
import tarfile
import tempfile
import zipfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
DIST_DIR = REPO_ROOT / "dist"
PACKAGE_MANIFEST = REPO_ROOT / "crates" / "santi-api" / "Cargo.toml"
CRATE_NAME = "santi-api"
BINARY_NAME = "santi-api"
ARCHIVE_PREFIX = "santi"


def main() -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    preflight_parser = subparsers.add_parser("preflight")
    preflight_parser.add_argument("--version", required=True)

    package_parser = subparsers.add_parser("package")
    package_parser.add_argument("--version", required=True)
    package_parser.add_argument("--target", required=True)

    args = parser.parse_args()

    if args.command == "preflight":
        preflight(args.version)
        return 0

    if args.command == "package":
        package(args.version, args.target)
        return 0

    raise SystemExit(f"unsupported command: {args.command}")


def preflight(version: str) -> None:
    parse_beta_version(version)
    manifest_version = read_manifest_version(PACKAGE_MANIFEST)
    if manifest_version != version:
        raise SystemExit(
            f"release version mismatch: got {version}, expected {manifest_version} from {PACKAGE_MANIFEST.relative_to(REPO_ROOT)}",
        )


def package(version: str, target: str) -> None:
    preflight(version)

    archive_stem = f"{ARCHIVE_PREFIX}-v{version}-{target}"
    DIST_DIR.mkdir(parents=True, exist_ok=True)

    run(["cargo", "build", "--release", "--locked", "--target", target, "-p", CRATE_NAME])

    with tempfile.TemporaryDirectory() as temp_dir:
        stage_dir = Path(temp_dir)
        binary_path = release_binary_path(target)
        staged_binary = stage_dir / binary_path.name
        shutil.copy2(binary_path, staged_binary)

        if "windows" in target:
            archive_path = DIST_DIR / f"{archive_stem}.zip"
            with zipfile.ZipFile(archive_path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
                archive.write(staged_binary, arcname=staged_binary.name)
        else:
            archive_path = DIST_DIR / f"{archive_stem}.tar.gz"
            with tarfile.open(archive_path, "w:gz") as archive:
                archive.add(staged_binary, arcname=staged_binary.name)


def release_binary_path(target: str) -> Path:
    binary_name = f"{BINARY_NAME}.exe" if "windows" in target else BINARY_NAME
    return REPO_ROOT / "target" / target / "release" / binary_name


def read_manifest_version(manifest_path: Path) -> str:
    in_package = False
    for raw_line in manifest_path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if line == "[package]":
            in_package = True
            continue
        if in_package and line.startswith("["):
            break
        if in_package and line.startswith("version"):
            _, value = line.split("=", 1)
            return value.strip().strip('"')

    raise SystemExit(f"failed to read package version from {manifest_path}")


def parse_beta_version(version: str) -> None:
    if not version.startswith("0.1.0-beta."):
        raise SystemExit(
            f"only long-lived beta versions 0.1.0-beta.N are allowed, got {version}",
        )

    try:
        int(version.removeprefix("0.1.0-beta."))
    except ValueError as error:
        raise SystemExit(
            f"only long-lived beta versions 0.1.0-beta.N are allowed, got {version}",
        ) from error


def run(args: list[str]) -> None:
    print(f"+ {' '.join(args)}")
    subprocess.run(args, cwd=REPO_ROOT, check=True)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as error:
        raise SystemExit(error.returncode) from error
