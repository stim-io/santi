#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
default_home="${HOME}/.santi-cli"
default_config_file="${default_home}/config.json"

config_file="${SANTI_CLI_CONFIG_FILE:-$default_config_file}"
config_dir="$(dirname "$config_file")"

base_url="${SANTI_CLI_BASE_URL:-http://127.0.0.1:18081}"

mkdir -p "$config_dir"

export SANTI_CLI_BASE_URL="$base_url"

python3 - "$config_file" <<'PY'
import json, os, sys
path = sys.argv[1]
data = {
    "base_url": os.environ["SANTI_CLI_BASE_URL"],
}
with open(path, "w", encoding="utf-8") as f:
    json.dump(data, f, indent=2)
    f.write("\n")
PY

cargo install --path "$repo_root/../santi-cli/app" --force >/dev/null

cargo_bin_dir="${CARGO_HOME:-$HOME/.cargo}/bin"

"$cargo_bin_dir/santi-cli" health >/dev/null

cat <<EOF
santi-cli installed.
config: $config_file
binary: $cargo_bin_dir/santi-cli

next:
  "$cargo_bin_dir/santi-cli" health
EOF
