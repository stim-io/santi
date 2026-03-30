#!/usr/bin/env bash
set -euo pipefail

default_home="${HOME}/.santi-cli"
config_file="${SANTI_CLI_CONFIG_FILE:-$default_home/config.json}"

if cargo uninstall santi-cli >/dev/null 2>&1; then
  echo "uninstalled santi-cli"
else
  echo "santi-cli not installed"
fi

if [ -d "$default_home" ]; then
  rm -rf "$default_home"
  echo "removed $default_home"
else
  echo "$default_home already absent"
fi

if [ -f "$config_file" ] && [ "$config_file" != "$default_home/config.json" ]; then
  rm -f "$config_file"
  echo "removed $config_file"
fi
