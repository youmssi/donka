#!/usr/bin/env bash
# Measured performance (DNK-67), on an installation made from the published images and left
# running by `DONKA_INSTALL_DIR=<dir> scripts/check-install.sh <studio> <runtime> demo`:
#
#   scripts/check-performance.sh <installation folder> [full]
#
# 1. Each starter pack's entry decision, and retail-credit's credit-application, which calls the
#    demo's fake bureau (a mocked connector), through the staging Runtime: p50/p95/p99 at the
#    target rate, then the maximum throughput. The inputs are the packs' own test scenarios.
# 2. The decision-log feed keeps up: every decision answered at the target rate is in Studio's
#    log within the target delay.
# 3. Studio stops while the Runtime answers: decisions keep their latency and, within the
#    Runtime's queue, none is lost; beyond it, records are dropped (counted in the Runtime's log)
#    and decisions still keep their latency.
#
# The results go to performance.json (or $DONKA_PERFORMANCE_OUT) and are checked against the
# published targets (docs/performance.json) and the baseline measured on the reference machine:
# a measure more than 20% worse than its baseline fails. `full` runs longer, for the published
# figures; CI runs the short version. The Studio image's donka-admin generates the load.
set -euo pipefail

dir="${1:?usage: scripts/check-performance.sh <installation folder> [full]}"
full="${2:-}"
root="$(cd "$(dirname "$0")/.." && pwd)"
targets="$root/docs/performance.json"
out="${DONKA_PERFORMANCE_OUT:-$root/performance.json}"
STUDIO_URL=http://localhost:8080
RUNTIME_URL=http://localhost:8090
API="$STUDIO_URL/api/v1"
PASSWORD="installation check passphrase"
JAR="$(mktemp)"
work="$(mktemp -d)"
# The inputs, read by donka-admin in the Studio image, which runs as its own user.
chmod 755 "$work"
rate=$(jq -r .targets.runtime.rate "$targets")
if [ -n "$full" ]; then seconds=60; max_seconds=30; else seconds=15; max_seconds=10; fi

fail() {
  echo "check-performance: $*" >&2
  exit 1
}
cleanup() { rm -rf "$JAR" "$work"; }
trap cleanup EXIT
studio() {
  curl -sS --fail-with-body -b "$JAR" -c "$JAR" -H 'x-donka-csrf: 1' \
    -H 'content-type: application/json' -X "$1" "$API$2" ${3:+--data "$3"}
}
cd "$dir"
image="$(grep '^DONKA_REGISTRY=' .env | cut -d= -f2- || true)"
image="${image:-ghcr.io/youmssi}/donka-studio:$(grep '^DONKA_VERSION=' .env | cut -d= -f2-)"
# load <project> <decision> <inputs file> <--rate N | --concurrency N> <seconds> [warm-up]
load() {
  docker run --rm --network host -v "$work:/work:ro" -e DONKA_RUNTIME_TOKEN="${tokens[$1]}" \
    "$image" donka-admin load-test --url "$RUNTIME_URL/api/projects/$1/evaluate/$2" \
    --inputs "/work/$3" "$4" "$5" --seconds "$6" --warmup "${7:-3}"
}
# ready <project> <decision>: the release reaches the Runtime within a few seconds of its
# deployment.
ready() {
  for _ in $(seq 1 60); do
    curl -sf -X POST "$RUNTIME_URL/api/projects/$1/evaluate/$2" \
      -H "X-Access-Token: ${tokens[$1]}" -H 'content-type: application/json' \
      --data "$(jq -c '{context: .[0]}' "$work/$1-$2.json")" >/dev/null && return 0
    sleep 1
  done
  fail "$1/$2 is not served by the Runtime"
}
logged() { studio GET "/projects/${ids[$1]}/decision-log?limit=1" | jq .total; }
# wait_logged <project> <count>: seconds until the log holds <count> records, or fails after 60.
wait_logged() {
  local started
  started=$(date +%s.%N)
  for _ in $(seq 1 240); do
    if [ "$(logged "$1")" -ge "$2" ]; then
      echo "$(date +%s.%N) - $started" | bc | xargs printf '%.1f\n'
      return 0
    fi
    sleep 0.25
  done
  echo -1
}

studio POST /auth/sign-in "{\"email\": \"admin@donka.local\", \"password\": \"$PASSWORD\"}" >/dev/null

# Every pack live on staging, with a Runtime token; retail-credit is the demo's.
declare -A ids tokens
for pack in $(studio GET /packs | jq -r '.items[].key'); do
  id=$(studio GET "/projects/by-key/$pack" 2>/dev/null | jq -r '.id // empty' || true)
  if [ -z "$id" ]; then
    id=$(studio POST "/packs/$pack/import" "$(jq -n --arg k "$pack" '{key: $k, name: $k}')" | jq -r .project.id)
    release=$(studio POST "/projects/$id/releases" '{"bump": "minor", "notes": "Performance"}' | jq -r .id)
    studio POST "/projects/$id/environments/staging/deployments" "$(jq -n --arg id "$release" '{releaseId: $id}')" >/dev/null
  fi
  ids[$pack]=$id
  tokens[$pack]=$(studio POST "/projects/$id/environments/staging/tokens" '{"name": "performance"}' | jq -r .token)
done

# What to measure: each pack's entry decision, and the decision that calls the bureau.
measured=()
for pack in "${!ids[@]}"; do
  entry=$(jq -r .entry "$root/packs/$pack/pack.json")
  measured+=("$pack/$entry")
  for decision in $(jq -r '.decisions[]' "$root/packs/$pack/pack.json"); do
    grep -q '"donka.connector"' "$root/packs/$pack/decisions/$decision.json" \
      && [ "$decision" != "$entry" ] && measured+=("$pack/$decision")
  done
done
IFS=$'\n' measured=($(sort <<<"${measured[*]}")); unset IFS

results='{}'
# The feed keeps up: every decision answered is in the log within the target delay.
feed_pack=kyc-risk-rating
feed_decision=$(jq -r .entry "$root/packs/$feed_pack/pack.json")
jq --arg d "$feed_decision" '[.[] | select(.decision == $d) | .input]' \
  "$root/packs/$feed_pack/scenarios.json" > "$work/$feed_pack-$feed_decision.json"
ready "$feed_pack" "$feed_decision"
before=$(logged "$feed_pack")
run=$(load "$feed_pack" "$feed_decision" "$feed_pack-$feed_decision.json" --rate "$rate" "$seconds" 0)
lag=$(wait_logged "$feed_pack" $((before + $(jq .answered <<<"$run"))))
echo "check-performance: the feed recorded $(jq .answered <<<"$run") decisions, the last ${lag}s after the load"
results=$(jq --argjson s "$(jq .answered <<<"$run")" --argjson l "$lag" '.feed = {sent: $s, lagSeconds: $l}' <<<"$results")

# outage <seconds of load> <seconds Studio is stopped>: the Runtime's answers while Studio is
# away, and the records that reach the log once it is back.
outage() {
  local before run lost
  before=$(logged "$feed_pack")
  load "$feed_pack" "$feed_decision" "$feed_pack-$feed_decision.json" --rate "$rate" "$1" 0 > "$work/outage.json" &
  local loader=$!
  sleep 3
  docker compose stop app >/dev/null 2>&1
  sleep "$2"
  docker compose start app >/dev/null 2>&1
  wait "$loader"
  run=$(cat "$work/outage.json")
  for _ in $(seq 1 120); do curl -sf "$API/ready" >/dev/null && break; sleep 1; done
  studio POST /auth/sign-in "{\"email\": \"admin@donka.local\", \"password\": \"$PASSWORD\"}" >/dev/null
  # The Runtime retries with backoff (up to 30 s): give it time to deliver what it holds.
  wait_logged "$feed_pack" $((before + $(jq .answered <<<"$run"))) >/dev/null || true
  sleep 5
  lost=$(( before + $(jq .answered <<<"$run") - $(logged "$feed_pack") ))
  jq --argjson lost "$lost" '. + {lost: $lost}' <<<"$run"
}
short=$(outage 30 15)
echo "check-performance: Studio stopped 15 s within the Runtime's queue: $short"
# A queue of 500 records: a 15 s outage at the target rate overflows it.
printf 'services:\n  runtime:\n    environment:\n      DECISION_LOG__QUEUE_CAPACITY: "500"\n' \
  > docker-compose.performance.yml
COMPOSE_FILE=docker-compose.yml:docker-compose.performance.yml docker compose up -d runtime >/dev/null 2>&1
sleep 5
overflow=$(outage 30 15)
# The Runtime logs (as JSON) the running count at 1, 2, 4, 8…: the last is a lower bound.
dropped=$(docker compose logs --no-log-prefix runtime 2>&1 | grep -o '"dropped":[0-9]*' | cut -d: -f2 | sort -n | tail -1)
echo "check-performance: Studio stopped 15 s beyond the queue: $overflow; the Runtime logged ${dropped:-0} dropped"
rm docker-compose.performance.yml
docker compose up -d runtime >/dev/null 2>&1
results=$(jq --argjson s "$short" --argjson o "$overflow" --argjson d "${dropped:-0}" \
  '.outage = {withinQueue: $s, beyondQueue: ($o + {droppedLogged: $d})}' <<<"$results")

# Then each decision, at the target rate and as fast as the Runtime answers. The fastest runs
# overflow the Runtime's decision-log queue by design (thousands a second for seconds): they come
# last, after the feed is measured on a Runtime that holds no backlog.
for target in "${measured[@]}"; do
  pack="${target%/*}"; decision="${target#*/}"
  jq --arg d "$decision" '[.[] | select(.decision == $d) | .input]' \
    "$root/packs/$pack/scenarios.json" > "$work/$pack-$decision.json"
  [ "$(jq length "$work/$pack-$decision.json")" -gt 0 ] || fail "$target has no scenario to send"
  ready "$pack" "$decision"
  at_rate=$(load "$pack" "$decision" "$pack-$decision.json" --rate "$rate" "$seconds")
  maximum=$(load "$pack" "$decision" "$pack-$decision.json" --concurrency 32 "$max_seconds")
  connector=$(grep -q '"donka.connector"' "$root/packs/$pack/decisions/$decision.json" && echo true || echo false)
  echo "check-performance: $target at $rate/s: $at_rate; maximum: $maximum"
  results=$(jq --arg t "$target" --argjson r "$at_rate" --argjson m "$maximum" --argjson c "$connector" \
    '.runtime[$t] = {connector: $c, atRate: $r, maximum: $m}' <<<"$results")
done

results=$(jq --arg m "$(uname -m)" --arg c "$(nproc)" --arg full "${full:-short}" \
  '. + {machine: {cpus: ($c | tonumber), arch: $m}, run: $full}' <<<"$results")
echo "$results" > "$out"

# The published targets, then no regression beyond the margin against the baseline.
jq -e --slurpfile t "$targets" '
  ($t[0].targets) as $t
  | [ .runtime[] | select(.connector | not) | .atRate | (.p95_ms <= $t.runtime.p95_ms and .p99_ms <= $t.runtime.p99_ms and .errors == 0) ] | all' \
  "$out" >/dev/null || fail "a decision without connector misses p95 or p99 at $rate/s: $(jq -c .runtime "$out")"
jq -e --slurpfile t "$targets" '($t[0].targets.feed.lag_seconds) as $max | .feed.lagSeconds >= 0 and .feed.lagSeconds <= $max' \
  "$out" >/dev/null || fail "the feed did not keep up: $(jq -c .feed "$out")"
jq -e '.outage.withinQueue | .lost == 0 and .errors == 0' "$out" >/dev/null \
  || fail "records were lost, or decisions failed, while Studio was away: $(jq -c .outage.withinQueue "$out")"
jq -e '.outage.beyondQueue.errors == 0' "$out" >/dev/null \
  || fail "decisions failed while the Runtime's queue was full: $(jq -c .outage.beyondQueue "$out")"
jq -e --slurpfile t "$targets" '
  ($t[0].margin) as $margin | ($t[0].baseline // {}) as $b
  | [ .runtime | to_entries[] | select($b.runtime[.key]) | . as $e
      | ($b.runtime[$e.key]) as $base
      | ($e.value.atRate.p95_ms <= ([$base.atRate.p95_ms * (1 + $margin), $base.atRate.p95_ms + 2] | max))
        and ($e.value.atRate.p99_ms <= ([$base.atRate.p99_ms * (1 + $margin), $base.atRate.p99_ms + 2] | max))
        and ($e.value.maximum.throughput >= $base.maximum.throughput * (1 - $margin)) ] | all' \
  "$out" >/dev/null || fail "a measure regressed more than the margin against the baseline: $(jq -c .runtime "$out")"
echo "check-performance: the targets hold, and no measure regressed beyond the margin ($out)"
