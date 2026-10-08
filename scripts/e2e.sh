#!/usr/bin/env bash
# End to end: a release made in Studio reaches Donka Runtime and answers with a token, its
# "bureau score" connector calls the bureau from the Runtime only, a CI pipeline pulls what is
# live on staging, and every decision the Runtime makes lands in Studio's decision log, where it
# is found, opened and replayed.
#
# Needs a running Studio (fresh database, DONKA_BOOTSTRAP_ADMIN_EMAIL set, its console output in
# STUDIO_LOG) and scripts/fake-bureau.py at BUREAU_URL with FAKE_BUREAU_KEY set. The script
# starts the Runtime itself (RUNTIME_BIN, output in RUNTIME_LOG) once it has a decision-log
# token for it; the Runtime reads the rest of its settings from this environment: the same
# bucket with PROVIDER__PREFIX=staging/, and DONKA_SECRET_BUREAU_API_KEY. BUREAU_SECRET is that
# key's value: the test checks it never leaves the Runtime.
#
#   STUDIO_URL=http://localhost:8080 RUNTIME_URL=http://localhost:3000 RUNTIME_BIN=... \
#   RUNTIME_LOG=/tmp/runtime.log BUREAU_URL=http://127.0.0.1:8099 BUREAU_SECRET=... \
#   STUDIO_LOG=/tmp/studio.log scripts/e2e.sh
set -euo pipefail

: "${STUDIO_URL:?set STUDIO_URL}"
: "${RUNTIME_URL:?set RUNTIME_URL}"
: "${STUDIO_LOG:?set STUDIO_LOG}"
: "${BUREAU_URL:?set BUREAU_URL}"
: "${BUREAU_SECRET:?set BUREAU_SECRET}"
: "${RUNTIME_BIN:?set RUNTIME_BIN}"
: "${RUNTIME_LOG:?set RUNTIME_LOG}"
API="$STUDIO_URL${DONKA_API_BASE_PATH:-/api/v1}"
PASSWORD="end-to-end passphrase"
FIXTURE="$(dirname "$0")/../crates/engine/tests/fixtures/table.json"
SCORE="$(dirname "$0")/e2e-person-score.json"
JAR="$(mktemp)"
RUNTIME_PID=
trap 'rm -f "$JAR"; [ -z "$RUNTIME_PID" ] || kill "$RUNTIME_PID"' EXIT

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

# A decision-log token for staging, then the staging Runtime with it.
log_token=$(studio POST /decision-log/tokens '{"environment": "staging", "name": "e2e-runtime"}' | jq -r .token)
DECISION_LOG__URL="$API/decision-log/records" DECISION_LOG__TOKEN="$log_token" DECISION_LOG__FLUSH_INTERVAL=200 \
  "$RUNTIME_BIN" > "$RUNTIME_LOG" 2>&1 &
RUNTIME_PID=$!
for _ in $(seq 1 60); do
  curl -sf "$RUNTIME_URL/api/health" >/dev/null && break
  sleep 1
done
curl -sf "$RUNTIME_URL/api/health" >/dev/null || fail "the Runtime did not start (see $RUNTIME_LOG)"

project=$(studio POST /projects '{"key": "e2e-credit", "name": "E2E credit"}' | jq -r .id)
P="/projects/$project"
# The bureau's score is each decision's outcome in the log.
studio PUT "$P/decision-log/settings" '{"outcomeField": "bureau.score"}' >/dev/null
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
# CI pulls what is live on staging with a project CI token, checksum verified.
ci_token=$(studio POST "$P/ci-tokens" '{"name": "e2e pipeline"}' | jq -r .token)
pulled=$(curl -sS --fail-with-body -X POST "$API/rules-sync" -H "Authorization: Bearer $ci_token" \
  -H 'content-type: application/json' --data '{"deployments": [{"project": "e2e-credit", "target": "env:staging"}]}')
[ "$(jq -r '.deployments[0].action' <<<"$pulled")" = load ] || fail "CI could not resolve staging: $pulled"
artifact=$(mktemp)
curl -sS --fail-with-body -H "Authorization: Bearer $ci_token" -o "$artifact" \
  "$API$(jq -r '.deployments[0].artifact.url' <<<"$pulled")"
[ "$(sha256sum "$artifact" | cut -d' ' -f1)" = "$(jq -r '.deployments[0].artifact.sha256' <<<"$pulled")" ] \
  || fail "the CI download does not match its checksum"
[ "$(unzip -p "$artifact" .config/project.json | jq -r .environment.key)" = staging ] \
  || fail "the CI artifact is not staging's"
rm -f "$artifact"

# The decision log: a decision with the caller's reference, the trace not asked for.
headers=$(mktemp)
logged=$(curl -sS --fail-with-body -D "$headers" -X POST "$RUNTIME_URL/api/projects/e2e-credit/evaluate/person-score" \
  -H 'content-type: application/json' -H "X-Access-Token: $token" -H 'X-Donka-Reference: E2E-0001' \
  --data '{"context": {"applicant": {"nationalId": "CM-1"}}}')
decision_id=$(grep -i '^x-decision-id:' "$headers" | cut -d' ' -f2 | tr -d '\r')
rm -f "$headers"
[ -n "$decision_id" ] || fail "the Runtime did not name the logged decision: $logged"
jq -e 'has("trace") | not' <<<"$logged" >/dev/null || fail "the answer has a trace nobody asked for"

found=
for _ in $(seq 1 30); do
  found=$(studio GET "$P/decision-log?reference=E2E-0001")
  [ "$(jq .total <<<"$found")" = 1 ] && break
  sleep 1
done
[ "$(jq -r '.items[0].id' <<<"$found")" = "$decision_id" ] || fail "the decision did not reach the log: $found"
[ "$(jq -r '.items[0].outcome' <<<"$found")" = 712 ] || fail "unexpected outcome: $found"
[ "$(jq -r '.items[0].environment' <<<"$found")" = staging ] || fail "unexpected environment: $found"

record=$(studio GET "$P/decision-log/$decision_id")
[ "$(jq -cS .output.bureau <<<"$record")" = '{"available":true,"score":712}' ] || fail "unexpected record: $record"
[ "$(jq -r .trace.bureau.traceData.mode <<<"$record")" = live ] || fail "the record has no connector trace: $record"
[ "$(jq -r .releaseVersion <<<"$record")" = 1.0.0 ] || fail "the record does not name its release: $record"
grep -qF "$BUREAU_SECRET" <<<"$record" && fail "the bureau secret appears in the decision log"

# Replay answers the connector from the record: the bureau is not called again.
calls=$(curl -sS "$BUREAU_URL/calls" | jq .score)
replayed=$(studio POST "$P/decision-log/$decision_id/replay")
[ "$(jq .identical <<<"$replayed")" = true ] || fail "the replay differs: $replayed"
[ "$(curl -sS "$BUREAU_URL/calls" | jq .score)" = "$calls" ] || fail "the replay called the bureau"
studio GET "$P/audit?action=decision_record.replayed" | jq -e '.total == 1' >/dev/null \
  || fail "the replay is not in the audit log"

echo "e2e: release 1.0.0 is live on staging, the Runtime answers with its token, the bureau connector scores from the Runtime, CI pulls staging, and the decision is logged, found and replayed"
