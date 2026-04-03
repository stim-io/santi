#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"

python3 - "$repo_root" <<'PY'
from pathlib import Path
import re
import sys

root = Path(sys.argv[1])
ignored_dirs = {'.git', 'node_modules', 'target', 'dist'}
patterns = {
    '.rs': re.compile(r'#\s*\[ignore\]'),
    '.js': re.compile(r'(^|[^\w])(describe\.skip|it\.skip|test\.skip|xdescribe|xit|xtest|\.skip\()'),
    '.jsx': re.compile(r'(^|[^\w])(describe\.skip|it\.skip|test\.skip|xdescribe|xit|xtest|\.skip\()'),
    '.ts': re.compile(r'(^|[^\w])(describe\.skip|it\.skip|test\.skip|xdescribe|xit|xtest|\.skip\()'),
    '.tsx': re.compile(r'(^|[^\w])(describe\.skip|it\.skip|test\.skip|xdescribe|xit|xtest|\.skip\()'),
    '.mjs': re.compile(r'(^|[^\w])(describe\.skip|it\.skip|test\.skip|xdescribe|xit|xtest|\.skip\()'),
    '.cjs': re.compile(r'(^|[^\w])(describe\.skip|it\.skip|test\.skip|xdescribe|xit|xtest|\.skip\()'),
}

violations = []
for path in root.rglob('*'):
    if not path.is_file():
        continue
    if any(part in ignored_dirs for part in path.parts):
        continue
    pattern = patterns.get(path.suffix)
    if pattern is None:
        continue
    text = path.read_text(encoding='utf-8', errors='ignore')
    if pattern.search(text):
        violations.append(path.relative_to(root).as_posix())

if violations:
    print('skipped tests are not allowed:', file=sys.stderr)
    for path in violations:
        print(f'  - {path}', file=sys.stderr)
    raise SystemExit(1)
PY
