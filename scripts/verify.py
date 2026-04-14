#!/usr/bin/env python3

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
IGNORED_DIRS = {".git", "node_modules", "target", "dist", "__pycache__"}
SKIP_PATTERNS = {
    ".rs": re.compile(r"#\s*\[ignore\]"),
    ".js": re.compile(r"(^|[^\w])(describe\.skip|it\.skip|test\.skip|xdescribe|xit|xtest|\.skip\()"),
    ".jsx": re.compile(r"(^|[^\w])(describe\.skip|it\.skip|test\.skip|xdescribe|xit|xtest|\.skip\()"),
    ".ts": re.compile(r"(^|[^\w])(describe\.skip|it\.skip|test\.skip|xdescribe|xit|xtest|\.skip\()"),
    ".tsx": re.compile(r"(^|[^\w])(describe\.skip|it\.skip|test\.skip|xdescribe|xit|xtest|\.skip\()"),
    ".mjs": re.compile(r"(^|[^\w])(describe\.skip|it\.skip|test\.skip|xdescribe|xit|xtest|\.skip\()"),
    ".cjs": re.compile(r"(^|[^\w])(describe\.skip|it\.skip|test\.skip|xdescribe|xit|xtest|\.skip\()"),
}


def main() -> int:
    ensure_no_skipped_tests()
    run(["cargo", "fmt", "--all", "--check"])
    run(["cargo", "clippy", "--workspace", "--locked", "--all-targets"])
    run(["cargo", "test", "--workspace", "--locked"])
    return 0


def ensure_no_skipped_tests() -> None:
    violations: list[str] = []

    for path in REPO_ROOT.rglob("*"):
        if not path.is_file():
            continue
        if any(part in IGNORED_DIRS for part in path.parts):
            continue

        pattern = SKIP_PATTERNS.get(path.suffix)
        if pattern is None:
            continue

        text = path.read_text(encoding="utf-8", errors="ignore")
        if pattern.search(text):
            violations.append(path.relative_to(REPO_ROOT).as_posix())

    if violations:
        lines = ["skipped tests are not allowed:", *[f"  - {path}" for path in violations]]
        raise SystemExit("\n".join(lines))


def run(args: list[str]) -> None:
    print(f"+ {' '.join(args)}")
    subprocess.run(args, cwd=REPO_ROOT, check=True)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as error:
        raise SystemExit(error.returncode) from error
