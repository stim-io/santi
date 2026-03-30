#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
default_home="${HOME}/.santi-cli"
default_config_file="${default_home}/config.json"

config_file="${SANTI_CLI_CONFIG_FILE:-$default_config_file}"
config_dir="$(dirname "$config_file")"

base_url="${SANTI_CLI_BASE_URL:-http://127.0.0.1:18081}"
backend="${SANTI_CLI_BACKEND:-local}"
database_url="${SANTI_CLI_DATABASE_URL:-postgres://santi:santi@127.0.0.1:15432/santi?sslmode=disable}"
redis_url="${SANTI_CLI_REDIS_URL:-redis://127.0.0.1:16379/0}"
runtime_root="${SANTI_CLI_RUNTIME_ROOT:-$default_home/runtime}"
execution_root="${SANTI_CLI_EXECUTION_ROOT:-$repo_root}"
openai_api_key="${SANTI_CLI_OPENAI_API_KEY:-codex-local-dev}"
openai_base_url="${SANTI_CLI_OPENAI_BASE_URL:-http://127.0.0.1:18082/openai/v1}"
openai_model="${SANTI_CLI_OPENAI_MODEL:-gpt-5.4}"

mkdir -p "$config_dir"
mkdir -p "$runtime_root"

export SANTI_CLI_BACKEND="$backend"
export SANTI_CLI_BASE_URL="$base_url"
export SANTI_CLI_DATABASE_URL="$database_url"
export SANTI_CLI_REDIS_URL="$redis_url"
export SANTI_CLI_RUNTIME_ROOT="$runtime_root"
export SANTI_CLI_EXECUTION_ROOT="$execution_root"
export SANTI_CLI_OPENAI_API_KEY="$openai_api_key"
export SANTI_CLI_OPENAI_BASE_URL="$openai_base_url"
export SANTI_CLI_OPENAI_MODEL="$openai_model"

python3 - "$config_file" <<'PY'
import json, os, sys
path = sys.argv[1]
data = {
    "backend": os.environ["SANTI_CLI_BACKEND"],
    "base_url": os.environ["SANTI_CLI_BASE_URL"],
    "database_url": os.environ["SANTI_CLI_DATABASE_URL"],
    "redis_url": os.environ["SANTI_CLI_REDIS_URL"],
    "runtime_root": os.environ["SANTI_CLI_RUNTIME_ROOT"],
    "execution_root": os.environ["SANTI_CLI_EXECUTION_ROOT"],
    "openai_base_url": os.environ["SANTI_CLI_OPENAI_BASE_URL"],
    "openai_model": os.environ["SANTI_CLI_OPENAI_MODEL"],
}
api_key = os.environ.get("SANTI_CLI_OPENAI_API_KEY", "")
if api_key:
    data["openai_api_key"] = api_key
with open(path, "w", encoding="utf-8") as f:
    json.dump(data, f, indent=2)
    f.write("\n")
PY

cargo install --path "$repo_root/crates/santi-cli" --force >/dev/null

cargo_bin_dir="${CARGO_HOME:-$HOME/.cargo}/bin"

"$cargo_bin_dir/santi-cli" health >/dev/null

cat <<EOF
santi-cli installed.
config: $config_file
binary: $cargo_bin_dir/santi-cli

next:
  "$cargo_bin_dir/santi-cli" health
EOF
