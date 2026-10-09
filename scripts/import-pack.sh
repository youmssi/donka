#!/usr/bin/env bash
# Imports a starter pack (packs/<name>) into Studio as a new project: its decisions, a first
# version of each, its test scenarios and its decision-log settings. Then checks every scenario
# passes. The project is an ordinary one: change anything; the pack is never linked back.
#
# Importing the same pack again under another key gives a second, independent copy:
#
#   STUDIO_URL=http://localhost:8080 DONKA_EMAIL=admin@bank.example DONKA_PASSWORD=... \
#   scripts/import-pack.sh packs/retail-credit                      # project "retail-credit"
#   PROJECT_KEY=salary-advance PROJECT_NAME="Salary advance" scripts/import-pack.sh packs/retail-credit
#
# The account must be allowed to create projects. Needs curl and jq.
set -euo pipefail

pack="${1:?usage: scripts/import-pack.sh <pack directory>}"
: "${STUDIO_URL:?set STUDIO_URL, e.g. http://localhost:8080}"
: "${DONKA_EMAIL:?set DONKA_EMAIL}"
: "${DONKA_PASSWORD:?set DONKA_PASSWORD}"
API="$STUDIO_URL${DONKA_API_BASE_PATH:-/api/v1}"
manifest="$pack/pack.json"
[ -f "$manifest" ] || { echo "import-pack: no pack.json in $pack" >&2; exit 1; }
[ "$(jq -r .format "$manifest")" = 1 ] || { echo "import-pack: unknown pack format" >&2; exit 1; }
key="${PROJECT_KEY:-$(jq -r .key "$manifest")}"
name="${PROJECT_NAME:-$(jq -r .name.en "$manifest")}"

JAR="$(mktemp)"
trap 'rm -f "$JAR"' EXIT
fail() { echo "import-pack: $*" >&2; exit 1; }

# studio METHOD PATH [JSON]: the response body; fails on an HTTP error.
studio() {
  curl -sS --fail-with-body -b "$JAR" -c "$JAR" -X "$1" "$API$2" \
    -H 'content-type: application/json' -H 'x-donka-csrf: 1' ${3:+--data "$3"}
}

studio POST /auth/sign-in "$(jq -n --arg e "$DONKA_EMAIL" --arg p "$DONKA_PASSWORD" '{email: $e, password: $p}')" \
  >/dev/null || fail "could not sign in as $DONKA_EMAIL"
project=$(studio POST /projects "$(jq -n --arg k "$key" --arg n "$name" --arg d "$(jq -r .description.en "$manifest")" \
  '{key: $k, name: $n, description: $d}')") || fail "could not create the project $key: $project"
P="/projects/$(jq -r .id <<<"$project")"
echo "Project $key ($name)"

# Decisions in the order the manifest lists them: a decision another one calls comes first.
declare -A ids revisions
for decision in $(jq -r '.decisions[]' "$manifest"); do
  created=$(studio POST "$P/decisions" \
    "$(jq -n --arg k "$decision" --slurpfile c "$pack/decisions/$decision.json" '{key: $k, content: $c[0]}')") \
    || fail "could not create $decision: $created"
  ids[$decision]=$(jq -r .id <<<"$created")
  revisions[$decision]=$(jq -r .revision <<<"$created")
done
studio PUT "$P/decision-log/settings" "$(jq -c .decisionLog "$manifest")" >/dev/null

# Scenarios before versions: saving a version runs them.
count=0
while read -r scenario; do
  decision=$(jq -r .decision <<<"$scenario")
  studio POST "$P/test-scenarios" "$(jq -c --arg id "${ids[$decision]}" '{decisionId: $id, name, input, expected, match}' <<<"$scenario")" \
    >/dev/null || fail "could not create the scenario $(jq -r .name <<<"$scenario")"
  count=$((count + 1))
done < <(jq -c '.[]' "$pack/scenarios.json")
echo "  $count test scenarios"

# Saving a version runs every scenario of the project, so only the last save tells: before it,
# scenarios of decisions without a version yet report errors.
tests=
for decision in $(jq -r '.decisions[]' "$manifest"); do
  version=$(studio POST "$P/decisions/${ids[$decision]}/versions" \
    "$(jq -n --arg m "Imported from the $(jq -r .key "$manifest") pack" --argjson r "${revisions[$decision]}" '{message: $m, revision: $r}')")
  tests=$(jq -c .tests <<<"$version")
  echo "  $decision: version $(jq -r .number <<<"$version")"
  jq -r '.warnings[] | "    input field \(.path): \(.kind)"' <<<"$version"
done
echo "  tests: $tests"
[ "$(jq '.failed + .errors' <<<"$tests")" = 0 ] || fail "some test scenarios do not pass; see the decision's history in Studio"
echo "import-pack: $key is ready. Open it in Studio, read packs/$(basename "$pack")/guide.md, and adjust the policy to yours."
