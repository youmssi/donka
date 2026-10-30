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
# ghcr.io/youmssi/donka-runtime:<tag> (or under DONKA_REGISTRY, which init then writes to .env),
# as do postgres and mailpit: nothing is pulled, so the same check proves an installation from the
# offline bundle. Needs Docker with Compose, curl and jq. With DONKA_INSTALL_DIR, the installation
# is made in that empty folder and left running, for the upgrade check.
#
# It prints how long it took to reach the recorded decision, from init or from
# DONKA_INSTALL_STARTED (seconds since the epoch, when the images were pulled or loaded), and
# fails above DONKA_INSTALL_BUDGET seconds when that is set: the five minutes the story promises.
set -euo pipefail

studio_tag="${1:?usage: scripts/check-install.sh <studio image tag> <runtime image tag> [demo]}"
runtime_tag="${2:?usage: scripts/check-install.sh <studio image tag> <runtime image tag> [demo]}"
demo="${3:-}"
root="$(cd "$(dirname "$0")/.." && pwd)"
registry="${DONKA_REGISTRY:-ghcr.io/youmssi}"
dir="${DONKA_INSTALL_DIR:-$(mktemp -d)}"
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
  if [ -z "${DONKA_INSTALL_DIR:-}" ]; then
    (cd "$dir" && docker compose down -v >/dev/null 2>&1) || true
    rm -rf "$dir"
  fi
  rm -f "$JAR"
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
  "$registry/donka-studio:$studio_tag" \
  donka-admin init --studio-version "$studio_tag" --runtime-version "$runtime_tag" \
  ${DONKA_REGISTRY:+--registry "$DONKA_REGISTRY"} ${demo:+--demo}
# The secrets are files: only the installation's owner enters its folder, and no other file
# holds a secret's value.
[ "$(stat -c %a "$dir")" = 700 ] || fail "the installation folder is open to others"
[ "$(stat -c %a "$dir/secrets")" = 711 ] || fail "secrets/ can be listed"
for secret in "$dir"/secrets/*; do
  value=$(cat "$secret")
  for file in donka.yaml .env runtime.env; do
    if grep -qF "$value" "$dir/$file"; then fail "$file holds the secret $(basename "$secret")"; fi
  done
done

# A second run keeps everything; a replay from donka.yaml writes the same files and keeps the
# secrets.
before=$(cd "$dir" && sha256sum donka.yaml .env runtime.env secrets/*)
docker run --rm -u "$(id -u):$(id -g)" -v "$dir:/install" -w /install \
  "$registry/donka-studio:$studio_tag" donka-admin init >/dev/null
docker run --rm -u "$(id -u):$(id -g)" -v "$dir:/install" -w /install \
  "$registry/donka-studio:$studio_tag" donka-admin init --config donka.yaml >/dev/null
[ "$(cd "$dir" && sha256sum donka.yaml .env runtime.env secrets/*)" = "$before" ] \
  || fail "a second init or a replay changed the installation"

cd "$dir"
docker compose up -d --pull never
wait_for "$API/health"
wait_for "$RUNTIME_URL/api/health"

link=$(docker compose logs app | grep -o 'setup-password/?token=[A-Za-z0-9_-]*' | head -1)
[ -n "$link" ] || fail "no password-setup link in Studio's logs"
studio POST /auth/password-setup "{\"token\": \"${link#*token=}\", \"password\": \"$PASSWORD\"}" >/dev/null
# Nothing but that link is printed: no secret reaches the logs.
for secret in secrets/*; do
  if docker compose logs 2>&1 | grep -qF "$(cat "$secret")"; then
    fail "$(basename "$secret") appears in the logs"
  fi
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

# The doctor, with Studio's own settings and network: everything works; with the mail server
# stopped, it says so and fails.
docker compose run --rm --no-deps -T app donka-admin doctor \
  || fail "the doctor found a problem on a working installation"
docker compose stop mailpit >/dev/null 2>&1
report=$(docker compose run --rm --no-deps -T app donka-admin doctor) \
  && fail "the doctor passed without a mail server"
grep -q 'FAIL  mail' <<<"$report" || fail "the doctor did not name the mail server: $report"
docker compose start mailpit >/dev/null 2>&1
echo "check-install: the doctor checks the installation and names what fails"

# The support bundle: the logs, the doctor's report and the configuration, and not one secret
# value, token or email address.
docker run --rm -u "$(id -u):$(id -g)" --group-add "$(stat -c %g /var/run/docker.sock)" \
  -v /var/run/docker.sock:/var/run/docker.sock -v "$dir:/install" -w /install \
  "$registry/donka-studio:$studio_tag" donka-admin support-bundle \
  || fail "the support bundle could not be made"
bundle=$(ls "$dir"/support-*.tar.gz)
[ "$(stat -c %a "$bundle")" = 600 ] || fail "the support bundle is open to others"
unpacked=$(mktemp -d)
tar -xzf "$bundle" -C "$unpacked"
for file in logs/app.log logs/runtime.log logs/postgres.log doctor.txt versions.txt config/.env; do
  [ -s "$unpacked/donka-support/$file" ] || fail "the support bundle has no $file"
done
grep -q 'ok    database' "$unpacked/donka-support/doctor.txt" \
  || fail "the support bundle's doctor report is not the doctor's"
for secret in "$dir"/secrets/*; do
  if grep -rqF "$(cat "$secret")" "$unpacked"; then
    fail "$(basename "$secret") is in the support bundle"
  fi
done
grep -rqE 'dnk_[A-Za-z0-9_-]{8,}|admin@donka\.local' "$unpacked" \
  && fail "a token or an email address is in the support bundle"
rm -rf "$unpacked" "$bundle"
echo "check-install: the support bundle holds logs and reports, and no secret"
echo "check-install: installed from the images and a decision recorded in $took s"
[ -z "$budget" ] || [ "$took" -le "$budget" ] || fail "took $took s, over the budget of $budget s"
