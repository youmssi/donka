#!/usr/bin/env bash
# End to end: a release made in Studio reaches Donka Runtime and answers with a token, and its
# "bureau score" connector calls the bureau from the Runtime only.
#
# Needs a running Studio (fresh database, DONKA_BOOTSTRAP_ADMIN_EMAIL set, its console output in
# STUDIO_LOG), a Runtime reading the same bucket with PROVIDER__PREFIX=staging/ and
# DONKA_SECRET_BUREAU_API_KEY set, and scripts/fake-bureau.py at BUREAU_URL with that key in
# FAKE_BUREAU_KEY. BUREAU_SECRET is the same value: the test checks it never leaves the Runtime.
#
#   STUDIO_URL=http://localhost:8080 RUNTIME_URL=http://localhost:8090 \
#   BUREAU_URL=http://127.0.0.1:8099 BUREAU_SECRET=... STUDIO_LOG=/tmp/studio.log scripts/e2e.sh
set -euo pipefail

: "${STUDIO_URL:?set STUDIO_URL}"
: "${RUNTIME_URL:?set RUNTIME_URL}"
: "${STUDIO_LOG:?set STUDIO_LOG}"
: "${BUREAU_URL:?set BUREAU_URL}"
: "${BUREAU_SECRET:?set BUREAU_SECRET}"
API="$STUDIO_URL${DONKA_API_BASE_PATH:-/api/v1}"
PASSWORD="end-to-end passphrase"
FIXTURE="$(dirname "$0")/../crates/engine/tests/fixtures/table.json"
SCORE="$(dirname "$0")/e2e-person-score.json"
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

# The reference "bureau score" connector. In Studio it answers with its mock (score 1).
score=$(studio POST "$P/decisions" \
  "$(jq -n --slurpfile c "$SCORE" --arg url "$BUREAU_URL/score" \
    '{key: "person-score", content: ($c[0] | .nodes[1].content.config.url = $url)}')")
score_id=$(jq -r .id <<<"$score")
simulated=$(studio POST "$P/decisions/$score_id/simulate" '{"context": {"applicant": {"nationalId": "CM-1"}}}')
[ "$(jq -cS .result.bureau <<<"$simulated")" = '{"available":true,"score":1}' ] \
  || fail "Studio's simulation did not use the mock: $simulated"
studio POST "$P/decisions/$score_id/versions" \
  "$(jq -n --argjson r "$(jq .revision <<<"$score")" '{message: "Bureau score", revision: $r}')" >/dev/null

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

# The Runtime calls the bureau with the secret from its environment.
scored=$(curl -sS --fail-with-body -X POST "$RUNTIME_URL/api/projects/e2e-credit/evaluate/person-score" \
  -H 'content-type: application/json' -H "X-Access-Token: $token" \
  --data '{"context": {"applicant": {"nationalId": "CM-1"}}, "trace": true}')
[ "$(jq -cS .result.bureau <<<"$scored")" = '{"available":true,"score":712}' ] \
  || fail "the Runtime did not get the bureau's score: $scored"
[ "$(jq -r '.trace.bureau.traceData.mode' <<<"$scored")" = live ] || fail "no live connector trace: $scored"
grep -qF "$BUREAU_SECRET" <<<"$scored" && fail "the bureau secret appears in the Runtime's answer"
grep -qF "$BUREAU_SECRET" <<<"$(studio GET "$P/releases/$(jq -r .id <<<"$release")")" \
  && fail "the bureau secret appears in the release"

deployment=$(studio GET "$P/environments" | jq -r '.items[] | select(.environment == "staging") | .live.releaseVersion')
[ "$deployment" = "1.0.0" ] || fail "Studio does not show 1.0.0 live on staging"
echo "e2e: release 1.0.0 is live on staging, the Runtime answers with its token, and the bureau connector scores from the Runtime"
