#!/usr/bin/env bash
# Every image an installation runs, the demo's and the upgrade's tools included, as install/docker-compose.yml names them
# for these tags (DNK-64): what to pull, measure or bundle.
#
#   scripts/install-images.sh <studio image tag> <runtime image tag>
set -euo pipefail

studio_tag="${1:?usage: scripts/install-images.sh <studio image tag> <runtime image tag>}"
runtime_tag="${2:?usage: scripts/install-images.sh <studio image tag> <runtime image tag>}"
root="$(cd "$(dirname "$0")/.." && pwd)"
# Compose reads the installation's .env, which only `donka-admin init` writes: use an empty one.
project="$(mktemp -d)"
trap 'rm -rf "$project"' EXIT
cp "$root/apps/app/install/docker-compose.yml" "$project/"
touch "$project/.env"
DONKA_VERSION="$studio_tag" DONKA_RUNTIME_VERSION="$runtime_tag" DONKA_POSTGRES_PASSWORD=unused \
  docker compose --project-directory "$project" -f "$project/docker-compose.yml" --profile demo \
  --profile tools config --images | sort -u
