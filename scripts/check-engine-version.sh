#!/usr/bin/env bash
# Fails when Studio and the Runtime would evaluate with different zen-engine versions.
# Usage: scripts/check-engine-version.sh [path/to/donka-runtime]
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
studio=$(grep -E '^zen-engine *=' "$root/Cargo.toml" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+[^"]*' | head -1)
echo "studio  zen-engine $studio"
runtime_dir="${1:-$root/../donka-runtime}"
if [[ -f "$runtime_dir/Cargo.toml" ]]; then
  runtime=$(grep -E '^zen-engine *=' "$runtime_dir/Cargo.toml" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+[^"]*' | head -1)
  echo "runtime zen-engine $runtime"
  if [[ "$studio" != "$runtime" ]]; then
    echo "::error::zen-engine drift: studio $studio, runtime $runtime" >&2
    exit 1
  fi
else
  echo "runtime not checked out at $runtime_dir, skipping"
fi
