#!/usr/bin/env bash
# The installation from the published images (DNK-64), as a bank's team runs it: `donka-admin
# init` in an empty folder, `docker compose up`, the first administrator's password from the
# one-time link, a release served by the staging Runtime from the shared volume, and the
# Runtime's decision recorded in Studio's log through the token Studio wired at first start.
#
#   scripts/check-install.sh <studio image tag> <runtime image tag>
#
# The images must exist locally as ghcr.io/youmssi/donka-studio:<tag> and
# ghcr.io/youmssi/donka-runtime:<tag>. Needs Docker with Compose, curl and jq. Prints how long
# it took from init to the recorded decision.
set -euo pipefail

studio_tag="${1:?usage: scripts/check-install.sh <studio image tag> <runtime image tag>}"
runtime_tag="${2:?usage: scripts/check-install.sh <studio image tag> <runtime image tag>}"
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

started=$(date +%s)
docker run --rm -u "$(id -u):$(id -g)" -v "$dir:/install" -w /install \
  "ghcr.io/youmssi/donka-studio:$studio_tag" \
  donka-admin init --studio-version "$studio_tag" --runtime-version "$runtime_tag"
[ "$(stat -c %a "$dir/.env")" = 600 ] || fail ".env is not readable by its owner only"

# A second run keeps the secrets.
before=$(sha256sum "$dir/.env")
docker run --rm -u "$(id -u):$(id -g)" -v "$dir:/install" -w /install \
  "ghcr.io/youmssi/donka-studio:$studio_tag" donka-admin init >/dev/null
[ "$(sha256sum "$dir/.env")" = "$before" ] || fail "a second init changed .env"

cd "$dir"
docker compose up -d
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
echo "check-install: installed from the images and a decision recorded in $(( $(date +%s) - started )) s"
