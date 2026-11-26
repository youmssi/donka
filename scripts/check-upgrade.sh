#!/usr/bin/env bash
# `donka-admin upgrade` (DNK-64) as an operator runs it, against a test registry: an installation
# of 1.0.0, then upgrades to releases that must be refused, rolled back and carried out.
#
#   scripts/check-upgrade.sh <studio image> <runtime image>
#
# The two images are published as every test release (each a distinct image, with a label); the
# releases are:
#   1.2.0  unsigned                                   -> refused, nothing changed
#   9.0.0  its upgrade.json only upgrades from 8.x    -> refused, nothing changed
#   1.1.0  a Studio that never becomes ready          -> rolled back, data and versions as before
#   2.0.0  Studio and Runtime                         -> upgraded, data kept, decisions still logged
#
# then unattended upgrades (`donka-admin auto-upgrade`), from 2.0.0:
#   refused while the installation is production; in its window, to 2.0.1 rather than the next
#   major version 3.0.0; then the auto-upgrade service itself moves to 2.0.2 and restarts on it.
#
# Needs Docker treating registry.test:5000 as an insecure registry, `127.0.0.1 registry.test` in
# /etc/hosts, cosign, curl and jq.
set -euo pipefail

studio_image="${1:?usage: scripts/check-upgrade.sh <studio image> <runtime image>}"
runtime_image="${2:?usage: scripts/check-upgrade.sh <studio image> <runtime image>}"
root="$(cd "$(dirname "$0")/.." && pwd)"
R=registry.test:5000/youmssi
work="$(mktemp -d)"
dir="$work/install"
API=http://localhost:8080/api/v1
PASSWORD="installation check passphrase"
JAR="$work/cookies"

fail() {
  echo "check-upgrade: $*" >&2
  cat "$dir"/upgrades/*.txt >&2 2>/dev/null || true
  (cd "$dir" && docker compose logs --tail 40 app runtime >&2) || true
  exit 1
}
cleanup() {
  (cd "$dir" && docker compose down -v >/dev/null 2>&1) || true
  docker rm -f donka-test-registry >/dev/null 2>&1 || true
  # The auto-upgrade service runs as root: its backups and reports are root's.
  docker run --rm -v "$work:/work" postgres:16-alpine rm -rf /work/install >/dev/null 2>&1 || true
  rm -rf "$work"
}
trap cleanup EXIT

docker run -d --name donka-test-registry -p 5000:5000 registry:2 >/dev/null
for _ in $(seq 1 30); do curl -sf http://registry.test:5000/v2/ >/dev/null && break; sleep 1; done

# publish <repository:tag> <base image> [Dockerfile lines...]
publish() {
  local context="$work/build-${1//[:\/]/-}"
  mkdir -p "$context"
  printf '{"upgradesFrom": ">=8.0.0", "runtime": ">=1.0.0"}\n' > "$context/upgrade-9.json"
  { echo "FROM $2"; echo "LABEL org.donka.test-release=$1"; printf '%s\n' "${@:3}"; } > "$context/Dockerfile"
  # The default builder sees the images Docker has locally.
  docker buildx build --builder default -q --load -t "$R/$1" "$context" >/dev/null
  docker push -q "$R/$1" >/dev/null
}
publish donka-studio:1.0.0 "$studio_image"
publish donka-runtime:1.0.0 "$runtime_image"
publish donka-studio:2.0.0 "$studio_image"
publish donka-runtime:2.0.0 "$runtime_image"
publish donka-studio:1.1.0 "$studio_image" 'CMD ["donka-admin", "no-such-command"]'
publish donka-studio:1.2.0 "$studio_image"
publish donka-studio:9.0.0 "$studio_image" 'COPY upgrade-9.json /etc/donka/upgrade.json'
publish donka-studio:2.0.1 "$studio_image"
publish donka-studio:3.0.0 "$studio_image"

(cd "$work" && COSIGN_PASSWORD=test cosign generate-key-pair >/dev/null 2>&1)
sign() {
  COSIGN_PASSWORD=test cosign sign --yes --key "$work/cosign.key" --tlog-upload=false \
    --allow-insecure-registry --allow-http-registry "$R/$1" >/dev/null 2>&1 || fail "cannot sign $1"
}
for image in donka-studio:2.0.0 donka-runtime:2.0.0 donka-studio:1.1.0 donka-studio:9.0.0 \
  donka-studio:2.0.1 donka-studio:3.0.0; do
  sign "$image"
done

mkdir -p "$dir"
DONKA_INSTALL_DIR="$dir" DONKA_REGISTRY="$R" "$root/scripts/check-install.sh" 1.0.0 1.0.0
cp "$work/cosign.pub" "$dir/"
docker network connect --alias registry.test donka_default donka-test-registry

studio() {
  curl -sS --fail-with-body -b "$JAR" -c "$JAR" -H 'x-donka-csrf: 1' \
    -H 'content-type: application/json' -X "$1" "$API$2" ${3:+--data "$3"}
}
recorded() {
  studio POST /auth/sign-in "{\"email\": \"admin@donka.local\", \"password\": \"$PASSWORD\"}" >/dev/null
  local project
  project=$(studio GET /projects/by-key/donka-smoke | jq -r .id)
  studio GET "/projects/$project/decision-log" | jq -r .total
}
running() { docker inspect --format '{{.Image}}' "$(cd "$dir" && docker compose ps -q "$1")"; }
image_id() { docker image inspect --format '{{.Id}}' "$R/$1"; }
version() { sed -n "s/^$1=//p" "$dir/.env"; }
# expect <exit status> <upgrade arguments...>
expect() {
  local status=0
  docker run --rm -u "$(id -u):$(id -g)" --group-add "$(stat -c %g /var/run/docker.sock)" \
    -v /var/run/docker.sock:/var/run/docker.sock -v "$dir:/install" -w /install \
    "$R/donka-studio:1.0.0" donka-admin upgrade "${@:2}" || status=$?
  [ "$status" = "$1" ] || fail "upgrade ${*:2} exited with $status, not $1"
  last_report=$(ls -t "$dir"/upgrades/*.txt | head -1)
}
# unattended <exit status>: one look by the installed version's `auto-upgrade --once`
unattended() {
  local status=0
  docker run --rm -u "$(id -u):$(id -g)" --group-add "$(stat -c %g /var/run/docker.sock)" \
    -v /var/run/docker.sock:/var/run/docker.sock -v "$dir:/install" -w /install \
    "$R/donka-studio:$(version DONKA_VERSION)" donka-admin auto-upgrade --once || status=$?
  [ "$status" = "$1" ] || fail "auto-upgrade --once exited with $status, not $1"
}
# configure <answer> <value>: changes an answer in donka.yaml and applies it, as an operator does
configure() {
  sed -i "/^$1:/d" "$dir/donka.yaml" && echo "$1: '$2'" >> "$dir/donka.yaml"
  docker run --rm -u "$(id -u):$(id -g)" -v "$dir:/install" -w /install \
    "$R/donka-studio:$(version DONKA_VERSION)" donka-admin init --config donka.yaml >/dev/null
}
unchanged() {
  [ "$(version DONKA_VERSION)" = 1.0.0 ] || fail "$1: .env names Studio $(version DONKA_VERSION)"
  [ "$(running app)" = "$(image_id donka-studio:1.0.0)" ] || fail "$1: Studio is not 1.0.0"
  [ "$(running runtime)" = "$(image_id donka-runtime:1.0.0)" ] || fail "$1: the Runtime is not 1.0.0"
}

before=$(recorded)
[ "$before" -ge 1 ] || fail "no decision recorded before the upgrades"

expect 3 1.2.0
grep -q "has no cosign signature" "$last_report" || fail "1.2.0 was not refused for its signature"
[ ! -e "$dir/backups" ] || fail "a refused upgrade took a backup"
unchanged "unsigned"
echo "check-upgrade: an unsigned release is refused"

expect 3 9.0.0
grep -q "upgrades Studio >=8.0.0, not 1.0.0" "$last_report" || fail "9.0.0 was not refused as incompatible"
unchanged "incompatible"
echo "check-upgrade: an incompatible release is refused"

expect 4 1.1.0
grep -q "FAIL upgrade Studio" "$last_report" || fail "1.1.0 did not fail its smoke test"
grep -q "ok   restore the previous versions" "$last_report" || fail "1.1.0 was not restored"
unchanged "rolled back"
[ "$(recorded)" = "$before" ] || fail "the rollback lost decision records"
echo "check-upgrade: a release that fails its smoke test is rolled back, data kept"

expect 0 2.0.0 --runtime-version 2.0.0
grep -q "Outcome: Upgraded" "$last_report" || fail "2.0.0 did not report its upgrade"
[ "$(version DONKA_VERSION)" = 2.0.0 ] && [ "$(version DONKA_RUNTIME_VERSION)" = 2.0.0 ] \
  || fail ".env does not name 2.0.0"
[ "$(running app)" = "$(image_id donka-studio:2.0.0)" ] || fail "Studio does not run 2.0.0"
[ "$(running runtime)" = "$(image_id donka-runtime:2.0.0)" ] || fail "the Runtime does not run 2.0.0"
[ "$(recorded)" = "$before" ] || fail "the upgrade lost decision records"
backup=$(jq -r .backup "${last_report%.txt}.json")
[ -s "$dir/$backup/donka.dump" ] && [ -s "$dir/$backup/volumes.tar.gz" ] || fail "no backup in $backup"
# The upgraded Runtime still serves the release and its decisions still reach Studio.
STUDIO_URL=http://localhost:8080 RUNTIME_URL=http://localhost:8090 \
  DONKA_SMOKE_EMAIL=admin@donka.local DONKA_SMOKE_PASSWORD="$PASSWORD" "$root/scripts/smoke.sh" >/dev/null
for _ in $(seq 1 60); do [ "$(recorded)" -gt "$before" ] && break; sleep 1; done
[ "$(recorded)" -gt "$before" ] || fail "the upgraded Runtime's decisions do not reach Studio"
echo "check-upgrade: Studio and the Runtime upgraded to 2.0.0, data kept, decisions still recorded"

reports=$(ls "$dir"/upgrades/*.txt | wc -l)
unattended 3
[ "$(version DONKA_VERSION)" = 2.0.0 ] || fail "a production installation upgraded by itself"
[ "$(ls "$dir"/upgrades/*.txt | wc -l)" = "$reports" ] || fail "a production installation tried an upgrade"
echo "check-upgrade: a production installation never upgrades by itself"

# A window that opened a minute ago, on today's day (the runner's and the containers' UTC).
configure role staging
configure auto_upgrade "$(date -u -d '-1 minute' '+%a %H:%M')-$(date -u -d '+30 minutes' '+%H:%M')"
unattended 0
[ "$(version DONKA_VERSION)" = 2.0.1 ] || fail "the unattended upgrade went to $(version DONKA_VERSION), not 2.0.1"
[ "$(running app)" = "$(image_id donka-studio:2.0.1)" ] || fail "Studio does not run 2.0.1"
[ "$(version DONKA_RUNTIME_VERSION)" = 2.0.0 ] || fail "the Runtime moved without a later release"
echo "check-upgrade: in its window, a staging installation upgrades to 2.0.1, not the major 3.0.0"

configure auto_upgrade "$(date -u -d '+1 day' '+%a') 02:00-04:00"
unattended 0
[ "$(version DONKA_VERSION)" = 2.0.1 ] || fail "an upgrade ran outside the window"
echo "check-upgrade: outside its window, nothing happens"

# The service itself: it looks at once, upgrades, then is recreated on the release it installed.
publish donka-studio:2.0.2 "$studio_image"
sign donka-studio:2.0.2
configure auto_upgrade "$(date -u -d '-1 minute' '+%a %H:%M')-$(date -u -d '+30 minutes' '+%H:%M')"
(cd "$dir" && docker compose up -d auto-upgrade >/dev/null)
for _ in $(seq 1 120); do
  [ "$(version DONKA_VERSION)" = 2.0.2 ] && [ "$(running auto-upgrade 2>/dev/null)" = "$(image_id donka-studio:2.0.2)" ] && break
  sleep 5
done
[ "$(version DONKA_VERSION)" = 2.0.2 ] || fail "the auto-upgrade service did not upgrade to 2.0.2"
[ "$(running app)" = "$(image_id donka-studio:2.0.2)" ] || fail "Studio does not run 2.0.2"
[ "$(running auto-upgrade)" = "$(image_id donka-studio:2.0.2)" ] \
  || fail "the auto-upgrade service still runs the previous release"
[ "$(recorded)" -gt "$before" ] || fail "the unattended upgrades lost decision records"
echo "check-upgrade: the auto-upgrade service upgraded to 2.0.2 and restarted on it"
