#!/usr/bin/env bash
# End to end: a release made in Studio reaches Donka Runtime and answers with a token.
#
# Needs a running Studio (fresh database, DONKA_BOOTSTRAP_ADMIN_EMAIL set, its console output in
# STUDIO_LOG) and a Runtime reading the same bucket with PROVIDER__PREFIX=staging/.
#
#   STUDIO_URL=http://localhost:8080 RUNTIME_URL=http://localhost:8090 \
#   STUDIO_LOG=/tmp/studio.log scripts/e2e.sh
set -euo pipefail

: "${STUDIO_URL:?set STUDIO_URL}"
: "${RUNTIME_URL:?set RUNTIME_URL}"
: "${STUDIO_LOG:?set STUDIO_LOG}"
API="$STUDIO_URL${DONKA_API_BASE_PATH:-/api/v1}"
PASSWORD="end-to-end passphrase"
FIXTURE="$(dirname "$0")/../crates/engine/tests/fixtures/table.json"
JAR="$(mktemp)"
trap 'rm -f "$JAR"' EXIT

fail() { echo "e2e: $*" >&2; exit 1; }

# studio METHOD PATH [JSON]: the response body; fails on an HTTP error.
studio() {
  curl -sS --fail-with-body -b "$JAR" -c "$JAR" -X "$1" "$API$2" \
    -H 'content-type: application/json' -H 'x-donka-csrf: 1' ${3:+--data "$3"}
}

link=$(grep -o 'setup-password/?token=[A-Za-z0-9_-]*' "$STUDIO_LOG" | head -1)
[ -n "$link" ] || fail "no first-administrator link in $STUDIO_LOG"
email=$(grep -o 'First administrator created: [^ ]*' "$STUDIO_LOG" | head -1 | cut -d' ' -f4)
studio POST /auth/password-setup "$(jq -n --arg t "${link#*token=}" --arg p "$PASSWORD" '{token: $t, password: $p}')" >/dev/null
studio POST /auth/sign-in "$(jq -n --arg e "$email" --arg p "$PASSWORD" '{email: $e, password: $p}')" >/dev/null

project=$(studio POST /projects '{"key": "e2e-credit", "name": "E2E credit"}' | jq -r .id)
P="/projects/$project"
decision=$(studio POST "$P/decisions" "$(jq -n --slurpfile c "$FIXTURE" '{key: "bureau/normalize", content: $c[0]}')")
studio POST "$P/decisions/$(jq -r .id <<<"$decision")/versions" \
  "$(jq -n --argjson r "$(jq .revision <<<"$decision")" '{message: "First table", revision: $r}')" >/dev/null
release=$(studio POST "$P/releases" '{"bump": "minor", "notes": "End-to-end release"}')
[ "$(jq -r .version <<<"$release")" = "1.0.0" ] || fail "unexpected first version: $release"
token=$(studio POST "$P/environments/staging/tokens" '{"name": "e2e"}' | jq -r .token)
studio POST "$P/environments/staging/deployments" \
  "$(jq -n --arg id "$(jq -r .id <<<"$release")" '{releaseId: $id}')" >/dev/null

# The publisher puts the artifact in the bucket; the Runtime picks it up on its next poll.
evaluate() {
  curl -sS -o /dev/null -w '%{http_code}' -X POST \
    "$RUNTIME_URL/api/projects/e2e-credit/evaluate/bureau/normalize" \
    -H 'content-type: application/json' -H "X-Access-Token: $1" --data '{"context": {"input": 12}}'
}
for _ in $(seq 1 60); do
  [ "$(evaluate "$token")" = 200 ] && break
  sleep 1
done
answer=$(curl -sS --fail-with-body -X POST "$RUNTIME_URL/api/projects/e2e-credit/evaluate/bureau/normalize" \
  -H 'content-type: application/json' -H "X-Access-Token: $token" --data '{"context": {"input": 12}}')
[ "$(jq -c .result <<<"$answer")" = '{"output":10}' ] || fail "unexpected answer: $answer"

[ "$(evaluate "dnk_not-a-token")" = 401 ] || fail "the Runtime accepted an unknown token"

deployment=$(studio GET "$P/environments" | jq -r '.[] | select(.environment == "staging") | .live.releaseVersion')
[ "$deployment" = "1.0.0" ] || fail "Studio does not show 1.0.0 live on staging"
echo "e2e: release 1.0.0 is live on staging and the Runtime answers with its token"
