#!/usr/bin/env bash
# Checks a built Studio image: it runs as a non-root user, starts against PostgreSQL, answers
# /api/v1/health, reports the SERVICE_VERSION it was built with on /api/v1/version, trusts
# Donka's licence key rather than the development one, and does not contain the licence issuer.
#
#   scripts/check-image.sh <image> <expected version>
#
# Needs PostgreSQL on localhost:5432 (user, password and database `donka`), as in CI.
set -euo pipefail

image="${1:?usage: scripts/check-image.sh <image> <expected version>}"
expected="${2:?usage: scripts/check-image.sh <image> <expected version>}"
name="donka-image-check-$$"
port="${CHECK_IMAGE_PORT:-18080}"

fail() { echo "check-image: $*" >&2; docker logs "$name" 2>&1 | tail -n 40 >&2 || true; exit 1; }
trap 'docker rm -f "$name" >/dev/null 2>&1 || true' EXIT

user=$(docker image inspect --format '{{.Config.User}}' "$image")
case "$user" in
  "" | root | 0 | 0:*) fail "the image runs as root (User: '${user}')" ;;
esac

docker run -d --name "$name" --network host \
  -e DONKA_LISTEN="127.0.0.1:$port" \
  -e DATABASE_URL=postgres://donka:donka@127.0.0.1:5432/donka \
  -e DONKA_PUBLIC_URL="http://localhost:$port" \
  -e DONKA_SMTP_URL=smtp://127.0.0.1:1025 -e 'DONKA_SMTP_FROM=Donka <donka@localhost>' \
  -e DONKA_STORAGE_URL=file:///tmp/releases \
  -e DONKA_DECISION_LOG_KEY="$(openssl rand -base64 32)" \
  -e DONKA_SECOND_STEP_KEY="$(openssl rand -base64 32)" \
  -e DONKA_PERSON_INDEX_KEY="$(openssl rand -base64 32)" \
  "$image" >/dev/null

for _ in $(seq 1 60); do
  curl -sf "localhost:$port/api/v1/health" >/dev/null && break
  sleep 1
done
curl -sf "localhost:$port/api/v1/health" >/dev/null || fail "no answer on /api/v1/health"

version=$(curl -sf "localhost:$port/api/v1/version" | jq -r .version)
[ "$version" = "$expected" ] || fail "/api/v1/version says '$version', expected '$expected'"

# The web app is served from the image too.
curl -sf "localhost:$port/" -o /dev/null || fail "the web app is not served"

# A delivered Studio trusts Donka's licence key only, and the licence issuer never ships (DNK-77).
if docker logs "$name" 2>&1 | grep -q "trusts the development licence key"; then
  fail "the image trusts the development licence key (built with development-licence-key)"
fi
probe=$(docker create "$image")
files=$(docker export "$probe" | tar -t)
docker rm "$probe" >/dev/null
if grep -q 'donka-licence$' <<<"$files"; then
  fail "the image contains the licence issuer"
fi
grep -qx 'usr/local/bin/donka-admin' <<<"$files" || fail "the image has no donka-admin"

echo "check-image: $image runs as '$user', is healthy and reports version $version"
