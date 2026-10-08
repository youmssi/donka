#!/usr/bin/env bash
# Checks a built Studio image: it runs as a non-root user, starts against PostgreSQL, answers
# /api/v1/health, and reports the SERVICE_VERSION it was built with on /api/v1/version.
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

echo "check-image: $image runs as '$user', is healthy and reports version $version"
