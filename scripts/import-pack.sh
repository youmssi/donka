#!/usr/bin/env bash
# Imports a pack (packs/<name>, or any folder in the pack format) into Studio as a new project,
# the way the web app imports a pack file: its decisions with a first version each, its test
# scenarios and its decision-log settings. Then checks every scenario passes. The project is an
# ordinary one: change anything; the pack is never linked back.
#
# Importing the same pack again under another key gives a second, independent copy:
#
#   STUDIO_URL=http://localhost:8080 DONKA_EMAIL=admin@bank.example DONKA_PASSWORD=... \
#   scripts/import-pack.sh packs/retail-credit                      # project "retail-credit"
#   PROJECT_KEY=salary-advance PROJECT_NAME="Salary advance" scripts/import-pack.sh packs/retail-credit
#
# The account must be an administrator. Needs curl, jq and zip.
set -euo pipefail

pack="${1:?usage: scripts/import-pack.sh <pack directory>}"
: "${STUDIO_URL:?set STUDIO_URL, e.g. http://localhost:8080}"
: "${DONKA_EMAIL:?set DONKA_EMAIL}"
: "${DONKA_PASSWORD:?set DONKA_PASSWORD}"
API="$STUDIO_URL${DONKA_API_BASE_PATH:-/api/v1}"
manifest="$pack/pack.json"
[ -f "$manifest" ] || { echo "import-pack: no pack.json in $pack" >&2; exit 1; }
key="${PROJECT_KEY:-$(jq -r .key "$manifest")}"
name="${PROJECT_NAME:-$(jq -r .name.en "$manifest")}"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
fail() { echo "import-pack: $*" >&2; exit 1; }

curl -sS --fail-with-body -c "$work/jar" -X POST "$API/auth/sign-in" \
  -H 'content-type: application/json' -H 'x-donka-csrf: 1' \
  --data "$(jq -n --arg e "$DONKA_EMAIL" --arg p "$DONKA_PASSWORD" '{email: $e, password: $p}')" \
  >/dev/null || fail "could not sign in as $DONKA_EMAIL"

(cd "$pack" && zip -qr "$work/pack.zip" .)
query=$(jq -rn --arg k "$key" --arg n "$name" '"key=\($k|@uri)&name=\($n|@uri)"')
imported=$(curl -sS --fail-with-body -b "$work/jar" -X POST "$API/packs/import?$query" \
  -H 'content-type: application/zip' -H 'x-donka-csrf: 1' \
  --data-binary "@$work/pack.zip") || fail "could not import $pack: $imported"

tests=$(jq -c .tests <<<"$imported")
echo "Project $key ($name): tests $tests"
[ "$(jq '.failed + .errors' <<<"$tests")" = 0 ] || fail "some test scenarios do not pass; see the decision's history in Studio"
echo "import-pack: $key is ready. Open it in Studio, read $pack/guide.md, and adjust the policy to yours."
