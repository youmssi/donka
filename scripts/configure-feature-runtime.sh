#!/usr/bin/env bash
# Issue a feed credential for the disposable feature-test Runtime. Never print its value.
set -euo pipefail
: "${STUDIO_URL:?set the disposable Studio origin}"
: "${DONKA_E2E_ADMIN_EMAIL:?set the test administrator}"
: "${DONKA_E2E_ADMIN_PASSWORD:?set its private test password}"
: "${DONKA_FEATURE_FEED_URL:?set the feed URL reachable from the test Runtime}"
root=$(cd "$(dirname "$0")/.." && pwd)
umask 077
jar=$(mktemp)
api="${STUDIO_URL%/}/api/v1"
signed_in=false
cleanup() {
  status=$?
  if "$signed_in"; then
    if ! curl -sS --fail-with-body -b "$jar" -X POST "$api/auth/sign-out" \
      -H 'x-donka-csrf: 1' >/dev/null; then
      status=1
    fi
  fi
  rm -f "$jar"
  exit "$status"
}
trap cleanup EXIT
curl -sS --fail-with-body -c "$jar" -X POST "$api/auth/sign-in" \
  -H 'content-type: application/json' -H 'x-donka-csrf: 1' \
  --data "$(jq -n --arg e "$DONKA_E2E_ADMIN_EMAIL" --arg p "$DONKA_E2E_ADMIN_PASSWORD" \
    '{email: $e, password: $p}')" >/dev/null
signed_in=true
issued=$(curl -sS --fail-with-body -b "$jar" -X POST "$api/decision-log/tokens" \
  -H 'content-type: application/json' -H 'x-donka-csrf: 1' \
  --data '{"environment":"staging","name":"Disposable feature Runtime"}')
token=$(jq -er '.token' <<<"$issued")
[[ "$token" =~ ^dnk_log_[A-Za-z0-9_-]+$ ]]
printf 'DECISION_LOG__URL=%s\nDECISION_LOG__TOKEN=%s\nDECISION_LOG__FLUSH_INTERVAL=200\n' \
  "$DONKA_FEATURE_FEED_URL" "$token" > "$root/.feature-runtime.env"
printf 'Disposable Runtime feed configured.\n'
