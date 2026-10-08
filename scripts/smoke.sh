#!/usr/bin/env bash
# Post-deploy smoke test of a Donka installation: Studio answers and reaches its database, the
# decision engine simulates, and a Runtime evaluates a release published by this Studio.
#
#   DONKA_SMOKE_EMAIL=admin@bank.example DONKA_SMOKE_PASSWORD=... \
#   STUDIO_URL=https://donka.bank.example RUNTIME_URL=https://runtime-staging.bank.example \
#   scripts/smoke.sh
#
# The account must be allowed to create projects. The evaluation uses its own project
# (DONKA_SMOKE_PROJECT, default `donka-smoke`) with one tiny decision: the first run creates it,
# releases it and deploys it to DONKA_SMOKE_ENVIRONMENT (default `staging`, the environment the
# Runtime at RUNTIME_URL serves); later runs reuse it. Each run issues a Runtime token for the
# evaluation and revokes it before exiting. Needs curl and jq.
set -euo pipefail

: "${STUDIO_URL:?set STUDIO_URL, e.g. http://localhost:8080}"
: "${RUNTIME_URL:?set RUNTIME_URL, e.g. http://localhost:8090}"
: "${DONKA_SMOKE_EMAIL:?set DONKA_SMOKE_EMAIL (an account allowed to create projects)}"
: "${DONKA_SMOKE_PASSWORD:?set DONKA_SMOKE_PASSWORD}"
API="$STUDIO_URL${DONKA_API_BASE_PATH:-/api/v1}"
PROJECT="${DONKA_SMOKE_PROJECT:-donka-smoke}"
ENVIRONMENT="${DONKA_SMOKE_ENVIRONMENT:-staging}"
# Seconds to wait for the Runtime to pick up a new deployment (it polls the bucket).
WAIT="${DONKA_SMOKE_WAIT_SECONDS:-90}"

JAR="$(mktemp)"
project_id=
token_id=
cleanup() {
  if [ -n "$token_id" ]; then
    studio DELETE "/projects/$project_id/environments/$ENVIRONMENT/tokens/$token_id" >/dev/null \
      || echo "smoke: could not revoke the smoke token $token_id; revoke it in Studio" >&2
  fi
  rm -f "$JAR"
}
trap cleanup EXIT

ok() { echo "  ok    $*"; }
fail() { echo "  FAIL  $*" >&2; exit 1; }

# studio METHOD PATH [JSON]: the response body; fails on an HTTP error.
studio() {
  curl -sS --fail-with-body -b "$JAR" -c "$JAR" -X "$1" "$API$2" \
    -H 'content-type: application/json' -H 'x-donka-csrf: 1' ${3:+--data "$3"}
}

# The decision: `doubled` is twice `amount`.
DECISION='{
  "nodes": [
    {"id": "in", "type": "inputNode", "name": "Request", "position": {"x": 0, "y": 0}},
    {"id": "double", "type": "expressionNode", "name": "Double", "position": {"x": 250, "y": 0},
     "content": {"expressions": [{"id": "e1", "key": "doubled", "value": "amount * 2"}]}},
    {"id": "out", "type": "outputNode", "name": "Response", "position": {"x": 500, "y": 0}}
  ],
  "edges": [
    {"id": "a", "sourceId": "in", "targetId": "double", "type": "edge"},
    {"id": "b", "sourceId": "double", "targetId": "out", "type": "edge"}
  ]
}'
KEY=smoke/double
CONTEXT='{"amount": 21}'
EXPECTED='{"doubled":42}'

echo "Studio $STUDIO_URL"
curl -sf "$API/health" >/dev/null || fail "health: no answer on $API/health"
ok "health"
curl -sf "$API/ready" >/dev/null || fail "ready: Studio cannot reach its database ($API/ready)"
ok "ready (database reachable)"
ok "version $(curl -sf "$API/version" | jq -r .version)"

studio POST /auth/sign-in \
  "$(jq -n --arg e "$DONKA_SMOKE_EMAIL" --arg p "$DONKA_SMOKE_PASSWORD" '{email: $e, password: $p}')" \
  >/dev/null || fail "sign-in as $DONKA_SMOKE_EMAIL"
simulated=$(studio POST /simulate \
  "$(jq -n --argjson d "$DECISION" --arg k "$KEY" --argjson c "$CONTEXT" '{decisions: {($k): $d}, key: $k, context: $c}')") \
  || fail "simulate: $simulated"
[ "$(jq -c .result <<<"$simulated")" = "$EXPECTED" ] || fail "simulate: unexpected result $simulated"
ok "simulate ($(jq -r .performance <<<"$simulated"))"

echo "Runtime $RUNTIME_URL"
curl -sf "$RUNTIME_URL/api/health" >/dev/null || fail "health: no answer on $RUNTIME_URL/api/health"
ok "health"

# The smoke project, released and live on the environment once.
if project=$(studio GET "/projects/by-key/$PROJECT" 2>/dev/null); then
  project_id=$(jq -r .id <<<"$project")
else
  project_id=$(studio POST /projects \
    "$(jq -n --arg k "$PROJECT" '{key: $k, name: "Smoke test", description: "Used by scripts/smoke.sh after each deploy."}')" \
    | jq -r .id) || fail "could not create the project $PROJECT"
  decision=$(studio POST "/projects/$project_id/decisions" \
    "$(jq -n --arg k "$KEY" --argjson c "$DECISION" '{key: $k, content: $c}')")
  studio POST "/projects/$project_id/decisions/$(jq -r .id <<<"$decision")/versions" \
    "$(jq -n --argjson r "$(jq .revision <<<"$decision")" '{message: "Smoke test", revision: $r}')" >/dev/null
  studio POST "/projects/$project_id/releases" '{"bump": "minor", "notes": "Smoke test"}' >/dev/null
  ok "created the project $PROJECT and its release"
fi
P="/projects/$project_id"
live=$(studio GET "$P/environments" | jq -r --arg e "$ENVIRONMENT" '.items[] | select(.environment == $e) | .live.releaseId // empty')
if [ -z "$live" ]; then
  release=$(studio GET "$P/releases" | jq -r '.items[0].id // empty')
  [ -n "$release" ] || fail "$PROJECT has no release"
  studio POST "$P/environments/$ENVIRONMENT/deployments" "$(jq -n --arg id "$release" '{releaseId: $id}')" >/dev/null \
    || fail "could not deploy $PROJECT to $ENVIRONMENT (production needs an approval: approve it once in Studio)"
  ok "deployed $PROJECT to $ENVIRONMENT"
fi

issued=$(studio POST "$P/environments/$ENVIRONMENT/tokens" '{"name": "smoke test (revoked after each run)"}')
token_id=$(jq -r .id <<<"$issued")
token=$(jq -r .token <<<"$issued")

# A new token reaches the Runtime with the next artifact the publisher writes, and the Runtime
# picks that up on its next poll.
answer=
for _ in $(seq 1 "$WAIT"); do
  answer=$(curl -sS -X POST "$RUNTIME_URL/api/projects/$PROJECT/evaluate/$KEY" \
    -H 'content-type: application/json' -H "X-Access-Token: $token" \
    --data "$(jq -n --argjson c "$CONTEXT" '{context: $c}')" || true)
  [ "$(jq -c .result <<<"$answer" 2>/dev/null)" = "$EXPECTED" ] && break
  sleep 1
done
[ "$(jq -c .result <<<"$answer" 2>/dev/null)" = "$EXPECTED" ] \
  || fail "evaluate: no expected answer from the Runtime within ${WAIT}s (last: $answer). Is it serving '$ENVIRONMENT' from Studio's bucket?"
ok "evaluate $PROJECT/$KEY on $ENVIRONMENT"

echo "smoke: Studio and the Runtime are working"
