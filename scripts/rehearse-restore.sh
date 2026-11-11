#!/usr/bin/env bash
# DNK-50 rehearsal: fresh install, pack transfer between installations, backup and
# restore of the whole installation (database, release and wiring volumes, secrets),
# sign-in with a second step after the restore, upgrade and rollback on the restored
# copy, and two Runtimes behind a load balancer. Everything runs in an isolated
# folder; an existing development or test stack is never touched.
#
# Isolation notes: the installation's compose file names its project `donka` and the
# upgrade tooling expects that name, so the two rehearsal installations take turns
# running it. Their data stays apart because an override file gives each one its own
# volumes (dnk50src_*, dnk50dst_*); before the upgrade phase the restored copy adopts
# the default volume names for the release and wiring volumes, exactly as a customer
# installation has them, so the upgrade's own backup covers them too. The existing
# `donka_postgres` volume (the development database) is never used or removed.
#
# Needs: Docker with Compose, curl, jq, python3, cosign on PATH (COSIGN_BIN
# overrides), and locally available images ghcr.io/youmssi/donka-studio:$TAG and
# ghcr.io/youmssi/donka-runtime:$TAG. A registry published on the host's port 5000
# serves the installation and the upgrade releases. It is reached as
# host.docker.internal:5000, a name the daemon, this shell and the upgrade's own
# container (which joins the installation's network to verify signatures) all
# resolve; Docker must treat it as an insecure (plain HTTP) registry, the same
# preparation scripts/check-upgrade.sh documents for its registry.
#
# Usage: rehearse-restore.sh [install-root]
#   TAG              release the rehearsal installs first (default: 1.0.0)
#   NEXT / BROKEN    releases the upgrade phase moves to / must roll back
#                    (defaults: 1.1.0 / 1.2.0)
#   DNK50_REGISTRY   registry prefix (default: host.docker.internal:5000/youmssi)
#   DNK50_KEEP=1     keep the installations and containers afterwards
#   DNK50_SKIP_BUILD=1  do not rebuild the two "next release" derivative images
#
# The script prints one line per timed phase and writes measurements to
# <install-root>/results.txt. Exit code is non-zero on the first failed check.
set -euo pipefail

TAG=${TAG:-1.0.0}
NEXT=${NEXT:-1.1.0}
BROKEN=${BROKEN:-1.2.0}
REGISTRY=${DNK50_REGISTRY:-host.docker.internal:5000/youmssi}
STUDIO_IMAGE=ghcr.io/youmssi/donka-studio:$TAG
RUNTIME_IMAGE=ghcr.io/youmssi/donka-runtime:$TAG
ROOT=${1:-"$HOME/dnk50-rehearsal"}
SRC=$ROOT/source
DST=$ROOT/target
BACKUP=$ROOT/backup
COSIGN_BIN=${COSIGN_BIN:-cosign}
ADMIN_EMAIL=admin@rehearsal.test
ADMIN_PASSWORD='Rehearsal-password-1'
APP=http://localhost:8080
RUNTIME=http://localhost:8090
RESULTS=$ROOT/results.txt
TAR_IMAGE=postgres:16-alpine

fail() { echo "rehearse-restore: $*" >&2; exit 1; }
say() { echo "rehearse-restore: $*"; }
now() { date +%s; }

for tool in docker curl jq python3; do
  command -v "$tool" >/dev/null || fail "$tool is required"
done
command -v "$COSIGN_BIN" >/dev/null || fail "cosign is required (set COSIGN_BIN)"
docker compose version >/dev/null 2>&1 || fail "docker compose is required"
docker image inspect "$STUDIO_IMAGE" >/dev/null 2>&1 || fail "image $STUDIO_IMAGE not found (build it first)"
docker image inspect "$RUNTIME_IMAGE" >/dev/null 2>&1 || fail "image $RUNTIME_IMAGE not found (build it first)"

# Volumes the upgrade tooling creates under the default names; remember whether
# they were already there so the teardown only removes what this run created.
had_releases_volume=0
had_wiring_volume=0
docker volume inspect donka_releases >/dev/null 2>&1 && had_releases_volume=1
docker volume inspect donka_wiring >/dev/null 2>&1 && had_wiring_volume=1

# A previous run may have stopped halfway: take its containers down and drop its
# volumes (all named after the rehearsal) so this run starts from nothing.
for d in "$SRC" "$DST"; do
  if [ -f "$d/docker-compose.yml" ]; then
    (cd "$d" && docker compose down >/dev/null 2>&1) || true
    (cd "$d" && DONKA_RUNTIME_IMAGE=unused docker compose -f docker-compose.yml \
      -f docker-compose.override.yml -f docker-compose.rehearsal.yml down \
      >/dev/null 2>&1) || true
  fi
done
docker volume rm -f dnk50src_postgres dnk50src_releases dnk50src_wiring \
  dnk50dst_postgres dnk50dst_releases dnk50dst_wiring >/dev/null 2>&1 || true

rm -rf "$ROOT"
mkdir -p "$SRC" "$DST" "$BACKUP"
: >"$RESULTS"
record() { echo "$*" | tee -a "$RESULTS"; }

# A code is accepted once: after submitting one, wait out its window before
# computing the next (at most 31 seconds).
next_window() { sleep $((30 - $(date +%s) % 30 + 1)); }

totp() { # totp <base32 key> -> current six-digit code
  python3 - "$1" <<'PY'
import base64, hashlib, hmac, struct, sys, time
key = base64.b32decode(sys.argv[1].replace(" ", "").upper())
msg = struct.pack(">Q", int(time.time()) // 30)
digest = hmac.new(key, msg, hashlib.sha1).digest()
o = digest[-1] & 15
print("%06d" % ((struct.unpack(">I", digest[o:o+4])[0] & 0x7FFFFFFF) % 1000000))
PY
}

admin() { # admin <install dir> <args...>: runs donka-admin from the installed release
  local dir=$1
  shift
  docker run --rm -u "$(id -u):$(id -g)" --group-add "$(stat -c %g /var/run/docker.sock)" \
    -v /var/run/docker.sock:/var/run/docker.sock -v "$dir:/install" -w /install \
    "$STUDIO_IMAGE" donka-admin "$@" </dev/null
}

compose() { local dir=$1; shift; (cd "$dir" && docker compose "$@"); }
up() { compose "$1" up -d --pull never >/dev/null; }
down() { compose "$1" down >/dev/null 2>&1 || true; }
wait_health() { # wait_health <base url> <what>
  local until_at=$((SECONDS + 240))
  until curl -sf -o /dev/null "$1/api/v1/health" 2>/dev/null; do
    [ $SECONDS -lt $until_at ] || fail "$2 did not answer on $1/api/v1/health"
    sleep 2
  done
}

cookie_jar=$ROOT/cookies.txt
sign_in() { # sign_in [code]: password step, then the second step when one is on
  rm -f "$cookie_jar"
  local status
  status=$(curl -s -o /tmp/dnk50-signin.json -w '%{http_code}' -c "$cookie_jar" \
    -X POST "$APP/api/v1/auth/sign-in" -H 'Content-Type: application/json' \
    -H 'x-donka-csrf: 1' \
    -d "{\"email\":\"$ADMIN_EMAIL\",\"password\":\"$ADMIN_PASSWORD\"}")
  if [ "$status" = 202 ]; then
    [ -n "${1:-}" ] || fail "sign-in asks for a second step but no code was given"
    curl -sf -b "$cookie_jar" -c "$cookie_jar" -X POST "$APP/api/v1/auth/sign-in/code" \
      -H 'Content-Type: application/json' -H 'x-donka-csrf: 1' \
      -d "{\"code\":\"$1\"}" >/dev/null \
      || fail "second-step code refused"
  elif [ "$status" != 200 ]; then
    fail "sign-in failed with HTTP $status: $(cat /tmp/dnk50-signin.json)"
  fi
}

install_at() { # install_at <dir> <volume prefix>
  docker run --rm -u "$(id -u):$(id -g)" -v "$1:/install" -w /install \
    "$STUDIO_IMAGE" donka-admin init --admin-email "$ADMIN_EMAIL" \
    --studio-version "$TAG" --runtime-version "$TAG" --registry "$REGISTRY" \
    --yes </dev/null >/dev/null
  cat >"$1/docker-compose.override.yml" <<EOF
volumes:
  postgres:
    name: $2_postgres
  releases:
    name: $2_releases
  wiring:
    name: $2_wiring
EOF
  up "$1"
  wait_health "$APP" "Studio"
  local link=
  for _ in $(seq 1 90); do
    link=$(compose "$1" logs app 2>/dev/null | grep -o 'setup-password/?token=[A-Za-z0-9_-]*' | head -1 || true)
    [ -n "$link" ] && break
    sleep 2
  done
  [ -n "$link" ] || fail "no password-setup link in Studio's logs in $1"
  curl -sf -X POST "$APP/api/v1/auth/password-setup" -H 'Content-Type: application/json' \
    -H 'x-donka-csrf: 1' \
    -d "{\"token\":\"${link#*token=}\",\"password\":\"$ADMIN_PASSWORD\"}" >/dev/null \
    || fail "first-run password setup failed in $1"
}

decision_total() { # decision_total <project id>: records Studio received for it
  curl -sf -b "$cookie_jar" "$APP/api/v1/projects/$1/decision-log" | jq .total
}

# Publish the two product images to the local registry once: the installations pull
# from there, and the upgrade drill publishes its releases to the same place.
for image in "donka-studio:$TAG" "donka-runtime:$TAG"; do
  docker tag "ghcr.io/youmssi/$image" "$REGISTRY/$image"
  docker push -q "$REGISTRY/$image" >/dev/null
done

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
run_smoke() {
  # Studio answers before its staging Runtime listens; the smoke test needs both.
  local until_at=$((SECONDS + 240))
  until curl -sf -o /dev/null "$RUNTIME/api/health" 2>/dev/null; do
    [ $SECONDS -lt $until_at ] || fail "the staging Runtime did not answer on $RUNTIME/api/health"
    sleep 2
  done
  STUDIO_URL=$APP RUNTIME_URL=$RUNTIME DONKA_SMOKE_EMAIL=$ADMIN_EMAIL \
    DONKA_SMOKE_PASSWORD=$ADMIN_PASSWORD "$SCRIPT_DIR/smoke.sh" >/dev/null
}

# ---------------------------------------------------------------- source installation
say "phase 1/6: fresh source installation"
t0=$(now)
install_at "$SRC" dnk50src
run_smoke || fail "smoke test failed on the source installation"
record "fresh install + smoke: $(( $(now) - t0 ))s"

# Enrol the administrator's second step, then prove a full sign-in with it.
sign_in
enrol=$(curl -sf -b "$cookie_jar" -H 'x-donka-csrf: 1' -X POST "$APP/api/v1/account/second-step/enrolment")
key=$(echo "$enrol" | jq -r .key)
[ -n "$key" ] && [ "$key" != null ] || fail "enrolment returned no key: $enrol"
curl -sf -b "$cookie_jar" -H 'x-donka-csrf: 1' -X POST "$APP/api/v1/account/second-step/enrolment/confirm" \
  -H 'Content-Type: application/json' -d "{\"code\":\"$(totp "$key")\"}" >/dev/null \
  || fail "second-step enrolment was refused"
next_window
sign_in "$(totp "$key")"
record "second step enrolled and used: ok"

# Export the smoke project as a pack from the source installation.
smoke_id=$(curl -sf -b "$cookie_jar" "$APP/api/v1/projects/by-key/donka-smoke" | jq -r .id)
curl -sf -b "$cookie_jar" "$APP/api/v1/projects/$smoke_id/export" \
  -o "$ROOT/rehearsal-pack.zip" || fail "pack export failed"
[ -s "$ROOT/rehearsal-pack.zip" ] || fail "exported pack is empty"
record "pack exported from source: $(stat -c %s "$ROOT/rehearsal-pack.zip") bytes"

# ---------------------------------------------------------------- fresh target + pack
say "phase 2/6: fresh target installation imports the exported pack"
t0=$(now)
down "$SRC"
install_at "$DST" dnk50dst
sign_in
curl -sf -b "$cookie_jar" -H 'x-donka-csrf: 1' -X POST \
  "$APP/api/v1/packs/import?key=donka-smoke&name=Smoke%20test" \
  -H 'Content-Type: application/zip' --data-binary @"$ROOT/rehearsal-pack.zip" >/dev/null \
  || fail "pack import failed on the target installation"
imported=$(curl -sf -b "$cookie_jar" "$APP/api/v1/projects" | jq '.items | length')
[ "$imported" -ge 1 ] || fail "no project on the target after the pack import"
record "fresh target + pack import: $(( $(now) - t0 ))s (projects on target: $imported)"

# ---------------------------------------------------------------- backup and restore
say "phase 3/6: back up the source, restore it onto the target"
down "$DST"
up "$SRC"
wait_health "$APP" "Studio (source, for backup)"
sign_in "$(totp "$key")"
source_decisions=
for _ in $(seq 1 30); do
  source_decisions=$(decision_total "$smoke_id")
  [ "${source_decisions:-0}" -ge 1 ] 2>/dev/null && break
  sleep 2
done
[ "${source_decisions:-0}" -ge 1 ] || fail "no decision recorded on the source before the backup"
t_backup=$(now)
compose "$SRC" exec -T postgres pg_dump -U donka -Fc donka >"$BACKUP/donka.dump"
docker run --rm -v dnk50src_releases:/data:ro -v "$BACKUP:/backup" "$TAR_IMAGE" \
  tar czf /backup/releases.tar.gz -C /data .
docker run --rm -v dnk50src_wiring:/data:ro -v "$BACKUP:/backup" "$TAR_IMAGE" \
  tar czf /backup/wiring.tar.gz -C /data .
cp -a "$SRC/secrets" "$BACKUP/secrets"
cp -a "$SRC/donka.yaml" "$SRC/.env" "$SRC/runtime.env" "$BACKUP/"
record "backup taken: $(( $(now) - t_backup ))s, dump $(stat -c %s "$BACKUP/donka.dump") bytes, releases $(stat -c %s "$BACKUP/releases.tar.gz") bytes"
record "source decision-log total at backup: $source_decisions"
down "$SRC"

t0=$(now)
rm -rf "$DST/secrets"
cp -a "$BACKUP/secrets" "$DST/secrets"
cp -a "$BACKUP/donka.yaml" "$BACKUP/.env" "$BACKUP/runtime.env" "$DST/"
docker volume rm -f dnk50dst_postgres dnk50dst_releases dnk50dst_wiring >/dev/null 2>&1 || true
docker volume create dnk50dst_releases >/dev/null
docker volume create dnk50dst_wiring >/dev/null
docker run --rm -v dnk50dst_releases:/data -v "$BACKUP:/backup" "$TAR_IMAGE" \
  sh -c 'tar xzf /backup/releases.tar.gz -C /data'
docker run --rm -v dnk50dst_wiring:/data -v "$BACKUP:/backup" "$TAR_IMAGE" \
  sh -c 'tar xzf /backup/wiring.tar.gz -C /data'
compose "$DST" up -d postgres >/dev/null
# The image's first start serves a temporary server while it initialises, then
# restarts: only trust the database when real queries keep succeeding.
ready=0
for _ in $(seq 1 90); do
  if compose "$DST" exec -T postgres psql -U donka -d donka -c 'SELECT 1' \
    >/dev/null 2>&1; then
    ready=$((ready + 1))
    [ "$ready" -ge 3 ] && break
  else
    ready=0
  fi
  sleep 2
done
[ "$ready" -ge 3 ] || fail "the target database never became ready"
compose "$DST" exec -T postgres pg_restore -U donka -d donka --clean --if-exists --no-owner \
  <"$BACKUP/donka.dump" || fail "database restore failed"
up "$DST"
wait_health "$APP" "Studio (restored)"
record "RTO (restore start to Studio answering): $(( $(now) - t0 ))s"

# The restored installation carries the source's data and its second step: sign in
# with a code from the key enrolled on the source.
sign_in "$(totp "$key")"
restored_decisions=$(decision_total "$smoke_id")
[ "$restored_decisions" = "$source_decisions" ] \
  || fail "decision-log total after restore is $restored_decisions, source had $source_decisions"
record "RPO check: decision-log total after restore = $restored_decisions (0 lost vs the backup)"
record "restored installation verified (sign-in with second step, decisions)"
# The smoke script signs in with a password only; turn the second step off now that
# its restore is proven, and keep it off for the remaining phases.
next_window
curl -sf -b "$cookie_jar" -H 'x-donka-csrf: 1' -X POST "$APP/api/v1/account/second-step/disable" \
  -H 'Content-Type: application/json' -d "{\"code\":\"$(totp "$key")\"}" >/dev/null \
  || fail "turning the second step off after its proof failed"
run_smoke || fail "smoke test failed on the restored installation"
record "smoke on restored installation: ok (second step turned off for the smoke script)"

# ------------------------------------------------------- adopt default volume names
# As a customer installation has them, so the upgrade's own backup covers the
# release and wiring volumes too. The database volume keeps its rehearsal name:
# the upgrade dumps and restores the database over the network, by service name.
say "phase 4/6: upgrade the restored installation, then a broken release rolls back"
down "$DST"
docker volume create donka_releases >/dev/null
docker volume create donka_wiring >/dev/null
docker run --rm -v dnk50dst_releases:/from:ro -v donka_releases:/to "$TAR_IMAGE" \
  sh -c 'tar cf - -C /from . | tar xf - -C /to'
docker run --rm -v dnk50dst_wiring:/from:ro -v donka_wiring:/to "$TAR_IMAGE" \
  sh -c 'tar cf - -C /from . | tar xf - -C /to'
cat >"$DST/docker-compose.override.yml" <<'EOF'
volumes:
  postgres:
    name: dnk50dst_postgres
  releases:
    name: donka_releases
  wiring:
    name: donka_wiring
EOF
up "$DST"
wait_health "$APP" "Studio (restored, default volume names)"

keydir=$ROOT/keys
mkdir -p "$keydir"
COSIGN_PASSWORD= "$COSIGN_BIN" generate-key-pair --output-key-prefix "$keydir/cosign" \
  </dev/null >/dev/null 2>&1
[ -s "$keydir/cosign.key" ] || fail "cosign key pair was not generated"
cp "$keydir/cosign.pub" "$DST/cosign.pub"
if [ "${DNK50_SKIP_BUILD:-}" != 1 ]; then
  docker tag "$REGISTRY/donka-studio:$TAG" "$REGISTRY/donka-studio:$NEXT"
  docker tag "$REGISTRY/donka-runtime:$TAG" "$REGISTRY/donka-runtime:$NEXT"
  printf 'FROM %s\nCMD ["donka-app", "--definitely-not-a-flag"]\n' "$REGISTRY/donka-studio:$TAG" \
    | docker build -q -t "$REGISTRY/donka-studio:$BROKEN" - >/dev/null
  printf 'FROM %s\nCMD ["donka-runtime", "--definitely-not-a-flag"]\n' "$REGISTRY/donka-runtime:$TAG" \
    | docker build -q -t "$REGISTRY/donka-runtime:$BROKEN" - >/dev/null
  for image in "donka-studio:$NEXT" "donka-runtime:$NEXT" "donka-studio:$BROKEN" "donka-runtime:$BROKEN"; do
    docker push -q "$REGISTRY/$image" >/dev/null
    # cosign runs in this shell, where the registry answers on loopback; it stores
    # the signature under the same repository, so the upgrade finds it through
    # $REGISTRY all the same.
    COSIGN_PASSWORD= "$COSIGN_BIN" sign --key "$keydir/cosign.key" --tlog-upload=false \
      --allow-insecure-registry --allow-http-registry --yes \
      "${DNK50_SIGN_REGISTRY:-localhost:5000/youmssi}/$image" >/dev/null 2>&1 \
      || fail "could not sign $image"
  done
fi
if admin "$DST" upgrade "$BROKEN" --runtime-version "$BROKEN" >"$ROOT/upgrade-broken.log" 2>&1; then
  fail "the broken release was not refused or rolled back (see upgrade-broken.log)"
fi
grep -q "^DONKA_VERSION=$TAG" "$DST/.env" \
  || fail ".env moved off $TAG after the broken release (see upgrade-broken.log)"
curl -sf "$APP/api/v1/health" >/dev/null || fail "Studio does not answer after the rollback"
record "broken release $BROKEN rolled back: Studio still answers on $TAG (log: upgrade-broken.log)"
admin "$DST" upgrade "$NEXT" --runtime-version "$NEXT" >"$ROOT/upgrade.log" 2>&1 \
  || fail "upgrade of the restored installation failed (see upgrade.log)"
grep -q "^DONKA_VERSION=$NEXT" "$DST/.env" || fail ".env does not name $NEXT after the upgrade"
wait_health "$APP" "Studio (upgraded)"
run_smoke || fail "smoke test failed after the upgrade"
record "upgrade on restored data carried out ($TAG -> $NEXT); smoke green"

# ---------------------------------------------------------------- two Runtimes + LB
say "phase 5/6: two Runtimes behind a load balancer"
cat >"$DST/docker-compose.rehearsal.yml" <<'YAML'
services:
  runtime2:
    image: ${DONKA_RUNTIME_IMAGE:?set DONKA_RUNTIME_IMAGE}
    restart: unless-stopped
    env_file: [runtime.env]
    volumes:
      - releases:/var/lib/donka/releases:ro
      - wiring:/var/lib/donka/wiring:ro
      - ./secrets:/run/secrets:ro
  lb:
    image: nginx:alpine
    restart: unless-stopped
    ports: ["127.0.0.1:8091:80"]
    volumes:
      - ./rehearsal-nginx.conf:/etc/nginx/conf.d/default.conf:ro
    depends_on: [runtime, runtime2]
YAML
cat >"$DST/rehearsal-nginx.conf" <<'NGINX'
upstream donka_runtime {
    least_conn;
    server runtime:8080;
    server runtime2:8080;
}
server {
    listen 80;
    location / {
        proxy_pass http://donka_runtime;
        proxy_set_header X-Access-Token $http_x_access_token;
        proxy_set_header Content-Type $content_type;
    }
}
NGINX
both() { compose "$DST" -f docker-compose.yml -f docker-compose.override.yml -f docker-compose.rehearsal.yml "$@"; }
DONKA_RUNTIME_IMAGE="$REGISTRY/donka-runtime:$NEXT" both up -d >/dev/null
LB=http://localhost:8091
for _ in $(seq 1 60); do curl -sf -o /dev/null "$LB/api/health" 2>/dev/null && break; sleep 2; done
curl -sf "$LB/api/health" >/dev/null || fail "the load balancer does not answer"

sign_in
issued=$(curl -sf -b "$cookie_jar" -H 'x-donka-csrf: 1' -X POST \
  "$APP/api/v1/projects/$smoke_id/environments/staging/tokens" \
  -H 'Content-Type: application/json' -d '{"name":"rehearsal (revoked at once)"}')
token=$(echo "$issued" | jq -r .token)
token_id=$(echo "$issued" | jq -r .id)
evaluate() {
  curl -sf -X POST "$LB/api/projects/donka-smoke/evaluate/smoke/double" \
    -H 'Content-Type: application/json' -H "X-Access-Token: $token" \
    -d '{"context":{"amount":21}}' | jq -c .result
}
# The new token reaches the Runtimes with the next published artifact, and one
# Runtime picks it up before the other: ask until two answers in a row are right,
# so both sides of the balancer hold the token before anything is stopped.
answers=0
for _ in $(seq 1 90); do
  if [ "$(evaluate 2>/dev/null || true)" = '{"doubled":42}' ]; then
    answers=$((answers + 1))
    [ "$answers" -ge 2 ] && break
  else
    answers=0
  fi
  sleep 2
done
[ "$(evaluate)" = '{"doubled":42}' ] || fail "no correct answer through the load balancer"
record "load balancer answers decisions"

# The balancer needs a few seconds to stop sending work to the stopped Runtime;
# decisions must come back within that failover window, and stay back.
answers_again() {
  local tries=$1
  for _ in $(seq 1 "$tries"); do
    [ "$(evaluate 2>/dev/null || true)" = '{"doubled":42}' ] && return 0
    sleep 2
  done
  return 1
}
DONKA_RUNTIME_IMAGE="$REGISTRY/donka-runtime:$NEXT" both stop runtime >/dev/null
answers_again 15 || fail "decisions stopped when one Runtime stopped"
record "one Runtime stopped: decisions keep answering"
DONKA_RUNTIME_IMAGE="$REGISTRY/donka-runtime:$NEXT" both stop app >/dev/null
sleep 2
[ "$(evaluate)" = '{"doubled":42}' ] || fail "decisions stopped when Studio stopped"
record "Studio stopped: decisions keep answering"
DONKA_RUNTIME_IMAGE="$REGISTRY/donka-runtime:$NEXT" both up -d >/dev/null
wait_health "$APP" "Studio (back up)"
sign_in
curl -sf -b "$cookie_jar" -H 'x-donka-csrf: 1' -X DELETE \
  "$APP/api/v1/projects/$smoke_id/environments/staging/tokens/$token_id" >/dev/null || true

# ---------------------------------------------------------------- wrap up
say "phase 6/6: measurements"
record "RPO: 0 decision records lost relative to the backup; the guide's daily-backup advice bounds RPO at 24h"
record "RTO measured above (restore start to Studio answering)"
if [ "${DNK50_KEEP:-}" != 1 ]; then
  DONKA_RUNTIME_IMAGE="$REGISTRY/donka-runtime:$NEXT" both down -v >/dev/null 2>&1 || true
  down "$SRC"
  compose "$SRC" down -v >/dev/null 2>&1 || true
  docker volume rm -f dnk50src_postgres dnk50src_releases dnk50src_wiring \
    dnk50dst_postgres dnk50dst_releases dnk50dst_wiring >/dev/null 2>&1 || true
  [ "$had_releases_volume" = 0 ] && docker volume rm -f donka_releases >/dev/null 2>&1 || true
  [ "$had_wiring_volume" = 0 ] && docker volume rm -f donka_wiring >/dev/null 2>&1 || true
  say "installations removed; backup and results stay in $ROOT"
else
  say "DNK50_KEEP=1: installations left running for inspection (source $SRC, target $DST)"
fi
say "rehearsal complete; measurements in $RESULTS"
