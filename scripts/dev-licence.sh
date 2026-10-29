#!/usr/bin/env bash
# Issues a licence signed with the development key (DNK-77), for local development and CI.
# Only builds with the `development-licence-key` feature trust it; a delivered Studio refuses it.
# Needs openssl (3.x), jq and uuidgen or /proc/sys/kernel/random/uuid.
#
#   scripts/dev-licence.sh <out file> [module...]     # modules: explain connectors analysis
#
# Without modules, the licence includes all three. It is valid from yesterday for a year.
set -euo pipefail

out="${1:?usage: scripts/dev-licence.sh <out file> [module...]}"
shift
modules=("$@")
[ ${#modules[@]} -gt 0 ] || modules=(explain connectors analysis)
root="$(cd "$(dirname "$0")/.." && pwd)"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

b64url_decode() {
  local text
  text=$(tr -d '[:space:]' | tr '_-' '/+')
  while [ $(( ${#text} % 4 )) -ne 0 ]; do text="$text="; done
  printf '%s' "$text" | base64 -d
}
b64url() { base64 -w0 | tr '+/' '-_' | tr -d '='; }

# PKCS#8 wrapping of the 32-byte Ed25519 seed, so openssl can sign with it.
{ printf '\x30\x2e\x02\x01\x00\x30\x05\x06\x03\x2b\x65\x70\x04\x22\x04\x20'
  b64url_decode < "$root/crates/licence/keys/development.key"; } > "$work/key.der"
openssl pkey -inform DER -in "$work/key.der" -out "$work/key.pem"
key_id=$(openssl pkey -in "$work/key.pem" -pubout -outform DER | tail -c 32 | sha256sum | cut -c1-16)

id=$(cat /proc/sys/kernel/random/uuid 2>/dev/null || uuidgen | tr 'A-Z' 'a-z')
jq -cn \
  --arg id "$id" \
  --arg issued "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg from "$(date -u -d yesterday +%F)" \
  --arg until "$(date -u -d '+1 year' +%F)" \
  --args '{id: $id, customer: "Donka development", issuedAt: $issued, validFrom: $from,
           validUntil: $until, modules: $ARGS.positional}' "${modules[@]}" \
  | tr -d '\n' > "$work/payload.json"
openssl pkeyutl -sign -inkey "$work/key.pem" -rawin -in "$work/payload.json" -out "$work/signature"

jq -n \
  --arg keyId "$key_id" \
  --arg payload "$(b64url < "$work/payload.json")" \
  --arg signature "$(b64url < "$work/signature")" \
  '{format: "donka-licence/1", keyId: $keyId, payload: $payload, signature: $signature}' > "$out"
echo "development licence written to $out (modules: ${modules[*]})" >&2
