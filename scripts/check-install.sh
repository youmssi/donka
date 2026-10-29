#!/usr/bin/env bash
# The installation from the published images (DNK-64), as a bank's team runs it: `donka-admin
# init` in an empty folder, `docker compose up`, the first administrator's password from the
# one-time link, a release served by the staging Runtime from the shared volume, and the
# Runtime's decision recorded in Studio's log through the token Studio wired at first start.
#
#   scripts/check-install.sh <studio image tag> <runtime image tag> [demo]
#
# With `demo`, the installation is prepared with `--demo`: the retail-credit pack must be live on
# staging from the first start, and its credit-application decision must call the bundled fake
# bureau through the Runtime (a national identifier ending in 9 has serious arrears there).
#
# The images must exist locally as ghcr.io/youmssi/donka-studio:<tag> and
# ghcr.io/youmssi/donka-runtime:<tag>, as do postgres and mailpit: nothing is pulled, so the same
# check proves an installation from the offline bundle. Needs Docker with Compose, curl and jq.
#
# It prints how long it took to reach the recorded decision, from init or from
# DONKA_INSTALL_STARTED (seconds since the epoch, when the images were pulled or loaded), and
# fails above DONKA_INSTALL_BUDGET seconds when that is set: the five minutes the story promises.
set -euo pipefail

studio_tag="${1:?usage: scripts/check-install.sh <studio image tag> <runtime image tag> [demo]}"
runtime_tag="${2:?usage: scripts/check-install.sh <studio image tag> <runtime image tag> [demo]}"
demo="${3:-}"
root="$(cd "$(dirname "$0")/.." && pwd)"
dir="$(mktemp -d)"
STUDIO_URL=http://localhost:8080
RUNTIME_URL=http://localhost:8090
API="$STUDIO_URL/api/v1"
PASSWORD="installation check passphrase"
JAR="$(mktemp)"

fail() {
  echo "check-install: $*" >&2
  (cd "$dir" && docker compose logs --tail 60 app runtime >&2) || true
  exit 1
}
cleanup() {
  (cd "$dir" && docker compose down -v >/dev/null 2>&1) || true
  rm -rf "$dir" "$JAR"
}
trap cleanup EXIT
studio() {
  curl -sS --fail-with-body -b "$JAR" -c "$JAR" -H 'x-donka-csrf: 1' \
    -H 'content-type: application/json' -X "$1" "$API$2" ${3:+--data "$3"}
}
wait_for() {
  for _ in $(seq 1 120); do curl -sf "$1" >/dev/null && return 0; sleep 1; done
  fail "no answer on $1"
}

started="${DONKA_INSTALL_STARTED:-$(date +%s)}"
budget="${DONKA_INSTALL_BUDGET:-}"
docker run --rm -u "$(id -u):$(id -g)" -v "$dir:/install" -w /install \
  "ghcr.io/youmssi/donka-studio:$studio_tag" \
  donka-admin init --studio-version "$studio_tag" --runtime-version "$runtime_tag" ${demo:+--demo}
[ "$(stat -c %a "$dir/.env")" = 600 ] || fail ".env is not readable by its owner only"

# A second run keeps the secrets.
before=$(sha256sum "$dir/.env")
docker run --rm -u "$(id -u):$(id -g)" -v "$dir:/install" -w /install \
  "ghcr.io/youmssi/donka-studio:$studio_tag" donka-admin init >/dev/null
[ "$(sha256sum "$dir/.env")" = "$before" ] || fail "a second init changed .env"

cd "$dir"
docker compose up -d --pull never
wait_for "$API/health"
wait_for "$RUNTIME_URL/api/health"

link=$(docker compose logs app | grep -o 'setup-password/?token=[A-Za-z0-9_-]*' | head -1)
[ -n "$link" ] || fail "no password-setup link in Studio's logs"
studio POST /auth/password-setup "{\"token\": \"${link#*token=}\", \"password\": \"$PASSWORD\"}" >/dev/null
# Nothing but that link is printed: no generated secret reaches the logs.
for key in DONKA_POSTGRES_PASSWORD DONKA_DECISION_LOG_KEY DONKA_SECOND_STEP_KEY DONKA_PERSON_INDEX_KEY; do
  value=$(sed -n "s/^$key=//p" .env)
  if docker compose logs 2>&1 | grep -qF "$value"; then fail "$key appears in the logs"; fi
done

STUDIO_URL="$STUDIO_URL" RUNTIME_URL="$RUNTIME_URL" \
  DONKA_SMOKE_EMAIL=admin@donka.local DONKA_SMOKE_PASSWORD="$PASSWORD" \
  "$root/scripts/smoke.sh"

# The staging Runtime sends its decisions with the token Studio wrote at first start.
studio POST /auth/sign-in "{\"email\": \"admin@donka.local\", \"password\": \"$PASSWORD\"}" >/dev/null
project=$(studio GET /projects/by-key/donka-smoke | jq -r .id)
recorded=
for _ in $(seq 1 60); do
  recorded=$(studio GET "/projects/$project/decision-log" | jq -r .total)
  [ "$recorded" -ge 1 ] 2>/dev/null && break
  sleep 1
done
[ "${recorded:-0}" -ge 1 ] || fail "the Runtime's decision did not reach Studio's log"
if [ -n "$demo" ]; then
  # The demo project was released to staging at the first start: call it through the Runtime.
  demo_project=$(studio GET /projects/by-key/retail-credit | jq -r .id)
  token=$(studio POST "/projects/$demo_project/environments/staging/tokens" '{"name": "install check"}' | jq -r .token)
  call() {
    curl -sS -X POST "$RUNTIME_URL/api/projects/retail-credit/evaluate/credit-application" \
      -H 'content-type: application/json' -H "X-Access-Token: $token" \
      --data "{\"context\": {\"applicant\": {\"nationalId\": \"$1\", \"age\": 38, \"employment\": \"public\",
        \"monthsInJob\": 96, \"monthlyIncome\": 450000, \"monthlyDebtPayments\": 40000,
        \"salaryDomiciled\": true}, \"loan\": {\"amount\": 2500000, \"termMonths\": 36}}}"
  }
  answer=
  for _ in $(seq 1 60); do
    answer=$(call CM-100)
    [ "$(jq -r '.result.decision // empty' <<<"$answer")" = approve ] && break
    sleep 1
  done
  [ "$(jq -r '.result.decision // empty' <<<"$answer")" = approve ] \
    || fail "the demo project is not served with a clean bureau report: $answer"
  answer=$(call CM-109)
  [ "$(jq -r .result.decision <<<"$answer")" = decline ] \
    || fail "the fake bureau's arrears did not decline CM-109: $answer"
  echo "check-install: the demo project calls the fake bureau through the Runtime"
fi
took=$(( $(date +%s) - started ))
echo "check-install: installed from the images and a decision recorded in $took s"
[ -z "$budget" ] || [ "$took" -le "$budget" ] || fail "took $took s, over the budget of $budget s"
