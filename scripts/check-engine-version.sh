#!/usr/bin/env bash
# Fails when Studio and Runtime use different versions or sources for rule evaluation.
# Usage: scripts/check-engine-version.sh [path/to/donka-runtime]
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
runtime_dir="${1:-$root/../donka-runtime}"

# Cargo.lock records what will actually build, including an exact Git commit for a patch.
engine_identity() {
  awk -F '"' '
    /^\[\[package\]\]$/ { name = "" }
    /^name = "/ { name = $2 }
    name ~ /^zen-(engine|expression|tmpl)$/ && /^version = "/ {
      if (++seen[name] > 1) invalid = 1
      version[name] = $2
    }
    name ~ /^zen-(engine|expression|tmpl)$/ && /^source = "/ { source[name] = $2 }
    END {
      split("zen-engine zen-expression zen-tmpl", names, " ")
      for (i = 1; i <= 3; i++) {
        name = names[i]
        if (!version[name] || !source[name] || version[name] != version["zen-engine"] ||
            source[name] != source["zen-engine"]) invalid = 1
      }
      if (invalid) exit 1
      print version["zen-engine"] " " source["zen-engine"]
    }
  ' "$1"
}

for lock in "$root/Cargo.lock" "$runtime_dir/Cargo.lock"; do
  if [[ ! -f "$lock" ]]; then
    echo "::error::Cargo.lock not found at $lock" >&2
    exit 1
  fi
done
if ! studio=$(engine_identity "$root/Cargo.lock"); then
  echo "::error::Studio must lock one shared source and version for zen-engine, zen-expression and zen-tmpl" >&2
  exit 1
fi
if ! runtime=$(engine_identity "$runtime_dir/Cargo.lock"); then
  echo "::error::Runtime must lock one shared source and version for zen-engine, zen-expression and zen-tmpl" >&2
  exit 1
fi
echo "studio  zen-engine $studio"
echo "runtime zen-engine $runtime"
if [[ "$studio" != "$runtime" ]]; then
  echo "::error::zen-engine drift: studio $studio, runtime $runtime" >&2
  exit 1
fi
