# Supply chain: provenance, Scorecard, image checks

**Written:** 2026-11-09 (DNK-73 part 4). Statuses are exact; what waits for a person or a
release says so.

## SLSA provenance — prepared, awaiting the first signed release

The published images will carry SLSA build level 3 provenance (in-toto attestation) from
the release workflow. The preparation is in place: builds run in GitHub-hosted isolation
from locked manifests, artifacts and images are cosign-signed at release, and every CI
action is pinned to a commit SHA (below). **What is missing is evidence, not machinery:**
no signed release has been published yet (the signing keys and `RELEASE_PLEASE_TOKEN` are
owner release items). Until the first signed release exists, no provenance is claimed.

When it exists, a bank verifies offline, without trusting Donka's servers:

```bash
# 1. Fetch the image and its attestation from the registry you mirror.
docker pull ghcr.io/youmssi/donka-studio:<version>
# 2. Verify the signature with the public key published on the Trust page.
cosign verify --key cosign.pub ghcr.io/youmssi/donka-studio:<version>
# 3. Verify the SLSA provenance attestation and its builder identity.
cosign verify-attestation --key cosign.pub \
  --type slsaprovenance ghcr.io/youmssi/donka-studio:<version>
```

Runtime release artifacts are verified the same way by construction: the Runtime refuses
any published artifact whose Ed25519 signature does not match the installation key
(DNK-87), before any decision is evaluated.

## OpenSSF Scorecard — running weekly

Scorecard analyses each public repository (Studio, Runtime, CLI) on a weekly schedule
and on every push to `develop`/`main`; results land in each repository's Security tab
(code scanning). Scores appear there after the first runs; this page does not quote a
score that has not been produced. The workflows themselves follow the rules they check:
read-only default permissions, all actions pinned to commit SHAs, Dependabot keeping the
pins fresh weekly (`.github/dependabot.yml`, `.github/workflows/scorecard.yml`).

## OpenSSF Best Practices badge — prepared, registration is the owner's

The badge is granted to a registered project on bestpractices.dev after its questionnaire
is answered. The answers are prepared from this dossier (licence clarity, documented build
and test practice, vulnerability reporting in `SECURITY.md`, signed releases, the ASVS and
SSDF pages). Registration needs the project owner's account, so it is an owner item;
until then the badge is **not obtained and not claimed**. The story criterion stays open
on exactly that point.

## CIS Docker Benchmark — image-level checks

The Benchmark mostly binds the bank's Docker host (daemon settings, swarm, logging):
those rows are the **customer's**, with the install guide pointing at them. The checks
that bind the images Donka publishes:

| CIS check (image/container) | Status | Evidence |
| --- | --- | --- |
| 4.1 Run containers as a non-root user | Met | Studio image is distroless `nonroot` (uid 65532), `USER nonroot`; CI `check-image.sh` fails the build if the image would run as root |
| 4.2 Images come from trusted registries only | Customer | The bank pulls from GHCR or mirrors into its own registry (install guides, incl. the offline bundle); which registries are trusted is the bank's daemon policy |
| 4.3 Do not install unnecessary packages | Met | Distroless final image: no shell, no package manager, one binary plus static files |
| 4.5 Image content trust (signing) | Partial | Artifacts are Ed25519-signed and Runtime-verified (DNK-87); image signing waits for the release keys (owner items) — same gate as SLSA above |
| 4.6 HEALTHCHECK in the image | Exception | The distroless image has no shell to probe with; liveness is the orchestrator's HTTP probe of `/api/v1/health` (checked in CI). Accepted: the check moves to the deployment, which the guides describe |
| 4.7 Do not use update instructions alone | Met | Dependencies rebuild from locked manifests (`--locked`, `--frozen-lockfile`); no `apt upgrade` layer exists |
| 4.9 Remove setuid/setgid permissions | Met | Distroless base carries none for application files; the application runs unprivileged with no setuid helper |
| 4.10 Do not store secrets in images | Met | `check-image.sh` refuses a delivered image built with the development licence feature; secrets live as files in the installation, never in layers |
| 5.x Runtime container settings (privileges, read-only FS, resource limits, no privileged ports) | Customer | Deployment choices in the bank's compose/orchestrator; the image needs no privilege, binds 8080 unprivileged, and the guides show the hardened settings |

## One rule under all of this

Nothing in this dossier is claimed from intention. A status moves only when its evidence
can be opened: a workflow run, a signature, a published score. The Trust page (DNK-66)
will link each result where a bank can check it without asking.
