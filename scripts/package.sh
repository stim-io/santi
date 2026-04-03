#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <version-tag> <target-triple>" >&2
  exit 1
fi

version="$1"
target="$2"
root_dir="$(cd "$(dirname "$0")/.." && pwd)"
dist_dir="$root_dir/dist"
rm -rf "$dist_dir"
mkdir -p "$dist_dir"

crate_name="santi-api"
binary_name="santi-api"
archive_basename="santi-${version}-${target}"

cd "$root_dir"
cargo build --release --locked --target "$target" -p "$crate_name"

stage_dir="$(mktemp -d)"
trap 'rm -rf "$stage_dir"' EXIT

if [[ "$target" == *windows* ]]; then
  cp "$root_dir/target/$target/release/${binary_name}.exe" "$stage_dir/${binary_name}.exe"
  (cd "$stage_dir" && zip -qr "$dist_dir/${archive_basename}.zip" .)
else
  cp "$root_dir/target/$target/release/${binary_name}" "$stage_dir/${binary_name}"
  tar -C "$stage_dir" -czf "$dist_dir/${archive_basename}.tar.gz" .
fi
