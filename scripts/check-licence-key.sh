#!/usr/bin/env bash
# Fails when a delivered Studio would trust a key that is not Donka's (DNK-77): the placeholder
# in crates/licence/keys/licence.pub, which nobody can sign for, or the development key.
#
#   scripts/check-licence-key.sh
#
# CI runs it on main, so no release is cut before Donka's own public key is committed.
set -euo pipefail
cd "$(dirname "$0")/.."

# The key id Studio computes: the first 16 hexadecimal digits of the SHA-256 of the key bytes.
key_id() {
  local key
  key=$(tr -d '[:space:]' < "$1" | tr '_-' '/+')
  while [ $(( ${#key} % 4 )) -ne 0 ]; do key="$key="; done
  printf '%s' "$key" | base64 -d | sha256sum | cut -c1-16
}

id=$(key_id crates/licence/keys/licence.pub)
placeholder=$(sed -n 's/^pub const PLACEHOLDER_KEY_ID: &str = "\([0-9a-f]*\)";$/\1/p' crates/licence/src/lib.rs)
[ -n "$placeholder" ] || { echo "check-licence-key: PLACEHOLDER_KEY_ID not found" >&2; exit 1; }

if [ "$id" = "$placeholder" ]; then
  echo "check-licence-key: crates/licence/keys/licence.pub is still the placeholder ($id)." >&2
  echo "Generate Donka's key pair with \`donka-licence keygen\` and commit its public key." >&2
  exit 1
fi
if [ "$id" = "$(key_id crates/licence/keys/development.pub)" ]; then
  echo "check-licence-key: crates/licence/keys/licence.pub is the development key ($id)." >&2
  exit 1
fi
echo "licence key $id"
