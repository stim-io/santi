#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "$0")/.." && pwd)"

bash "$root_dir/scripts/verify/no-skips.sh"
cd "$root_dir"

cargo fmt --all --check
cargo test --workspace --locked
