#!/usr/bin/env bash
# The offline bundle (DNK-64): everything a server without internet access needs to install one
# Donka version, in one file. A release attaches it, signed (images.yml).
#
#   scripts/bundle.sh <studio image tag> <runtime image tag> <output dir>
#
# It holds every image the installation runs (scripts/install-images.sh, the demo's included)
# in one archive, a list of those images with the registry digests their signatures cover, a
# README with the commands to run, and SHA256SUMS over all three. The images are taken
# from the local Docker store when they are there, otherwise pulled for linux/amd64. Signing is
# the caller's: `cosign sign-blob` over SHA256SUMS, written to SHA256SUMS.sig.
set -euo pipefail

studio_tag="${1:?usage: scripts/bundle.sh <studio image tag> <runtime image tag> <output dir>}"
runtime_tag="${2:?usage: scripts/bundle.sh <studio image tag> <runtime image tag> <output dir>}"
out="${3:?usage: scripts/bundle.sh <studio image tag> <runtime image tag> <output dir>}"
root="$(cd "$(dirname "$0")/.." && pwd)"
name="donka-$studio_tag-offline"
dir="$out/$name"
[ ! -e "$dir" ] || { echo "bundle: $dir already exists" >&2; exit 1; }

mapfile -t images < <("$root/scripts/install-images.sh" "$studio_tag" "$runtime_tag")
[ "${#images[@]}" -ge 4 ] || { echo "bundle: the installation's images could not be listed" >&2; exit 1; }
mkdir -p "$dir"
for image in "${images[@]}"; do
  docker image inspect "$image" >/dev/null 2>&1 || docker pull -q --platform linux/amd64 "$image"
  # The digest the registry serves, which cosign signed; none for an image built locally.
  digest=$(docker image inspect --format '{{ join .RepoDigests " " }}' "$image" | tr ' ' '\n' |
    grep -m1 "^${image%:*}@" | cut -d@ -f2 || true)
  printf '%s %s\n' "$image" "${digest:-unpublished}" >> "$dir/images.txt"
done
docker save "${images[@]}" | gzip -1 > "$dir/images.tar.gz"

cat > "$dir/README.txt" <<EOF
Donka Studio $studio_tag with Donka Runtime $runtime_tag, for a server without internet access
(linux/amd64, Docker Engine with the Compose plugin 2.24 or later).

1. Check the bundle with Donka's public key, cosign.pub, obtained from Donka rather than from
   this bundle. Either tool works:

     cosign verify-blob --key cosign.pub --insecure-ignore-tlog \\
       --signature SHA256SUMS.sig SHA256SUMS
     openssl dgst -sha256 -verify cosign.pub -signature <(base64 -d SHA256SUMS.sig) SHA256SUMS

   then the files it lists:

     sha256sum -c SHA256SUMS

2. Load the images (images.txt lists them, with the registry digests their own signatures
   cover):

     docker load -i images.tar.gz

3. Prepare and start the installation, in an empty folder:

     docker run --rm -u "\$(id -u):\$(id -g)" -v "\$PWD:/install" -w /install \\
       ghcr.io/youmssi/donka-studio:$studio_tag donka-admin init \\
       --studio-version $studio_tag --runtime-version $runtime_tag --offline \\
       --public-url https://donka.bank.example --admin-email you@bank.example
     docker compose up -d --pull never

   The rest is the installation guide: docs/install.md in Donka Studio's repository.
EOF
(cd "$dir" && sha256sum images.tar.gz images.txt README.txt > SHA256SUMS)
echo "$dir"
