# Coverage and dependency security

DNK-49 enforces a 70% executable-line floor for business logic in Studio, Runtime and CLI.
Keep meaningful behavior assertions and the live feature journeys: a percentage does not
prove a workflow works. Add tests when a gate fails; do not lower the floor, skip tests or
exclude a source file to make it pass.

## Coverage scope

| Repository | Measured business logic | Gate |
| --- | --- | --- |
| Studio Rust | The eleven business crates listed in `scripts/rust-coverage.json`: audit, decision, decision-log, engine, explain, identity, installation, licence, pack, project and release | Each crate independently reaches 70% |
| Studio web | All `.ts` files under `src/modules` and `src/components/shared/api`, including service responses, schemas, hooks and transformations | Combined executable lines reach 70% |
| Runtime | Connectors, contracts, release/access data, immutable loading, engine access, decision records, throttling, rule specifications, schema derivation, secret-file resolution, ZIP publication and prefix handling, listed in its `scripts/rust-coverage.json` | Each listed file or module independently reaches 70% |
| CLI | All executable `src/**/*.ts`, including commands, artifact handling, forms and the MCP bridge | Combined executable lines reach 70% |

Rust tests run across the whole workspace. Framework routing, process startup, telemetry,
database plumbing, mail and remote storage adapters retain compilation, integration and live
checks; their coverage does not dilute a business module's floor. The full LLVM report is
retained, including files outside the gate's module list. Unit-test bodies live in companion
`tests.rs` files: [cargo-llvm-cov excludes these from the report](https://github.com/taiki-e/cargo-llvm-cov#exclude-code-from-coverage),
while still running them. Keeping assertions outside production source avoids counting the
tests themselves toward a business module's floor on stable Rust.
The report checker also refuses inline test bodies in the policy's production files, preventing
later changes from inflating the denominator with test code.

Web rendering lives in `.tsx` and has component, Storybook and live-browser checks. Its
service and state logic in `.ts` remains in the line gate. Declaration files, tests and barrel
re-exports contribute no business behavior and are outside that measurement. New business
modules must join the applicable policy in the same story.

The Rust checker rejects malformed reports, duplicate files, missing modules and zero-line
modules. It compares exact covered/total counts, without rounding or pooling modules. Web
coverage uses Vitest's matching V8 provider. CLI tests collect V8 data from the built CLI
subprocesses and the actual MCP API bundle; source maps remap it to TypeScript. Its floor
uses the executable-line counts in `coverage/coverage-summary.json`. The native V8 text
table uses a different line definition and is not the gate. Never-imported CLI source files
are included at zero; a real bundled fixture verifies both source mapping and that rule.

## Run locally

Use the repository's pinned pnpm and a Rust toolchain with `llvm-tools-preview`. Rust tools:

```bash
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --version 0.9.1 --locked
cargo install cargo-audit --version 0.22.2 --locked
```

Studio, with a dedicated PostgreSQL service and its `DATABASE_URL`:

```bash
node --test ../donka-runtime/scripts/check-rust-coverage.test.mjs
mkdir -p coverage
cargo llvm-cov --workspace --locked --json --output-path coverage/rust.json
node ../donka-runtime/scripts/check-rust-coverage.mjs coverage/rust.json scripts/rust-coverage.json
cargo audit
pnpm --dir apps/web install --frozen-lockfile
pnpm --dir apps/web test:coverage
pnpm --dir apps/web audit:security
```

Runtime, with Docker available for the MinIO/SILO and Azurite integration tests, from its
own checkout:

```bash
mkdir -p coverage
cargo llvm-cov --workspace --locked --json --output-path coverage/rust.json
node --test scripts/check-rust-coverage.test.mjs
node scripts/check-rust-coverage.mjs coverage/rust.json scripts/rust-coverage.json
cargo audit
cargo test -p donka-connectors --locked
```

Studio CI checks out an exact public Runtime commit for that same checker. Runtime can run
its checks independently of Studio repository visibility. CLI, from its own checkout:

```bash
pnpm install --frozen-lockfile
pnpm test:coverage
pnpm audit:security
```

Run the remaining repository format, lint, type, build and live-test checks as documented in
its contribution guide. Do not overlap SQLx suites against the same PostgreSQL service.
Coverage artifacts are ignored locally and retained in CI for seven days. Raw browser traces,
service logs and credentials stay private.

## Dependency checks

`cargo audit` uses the committed `.cargo/audit.toml` high-severity policy. The Node checks use
`pnpm audit --audit-level high`, including development tools. These commands fail on high or
critical advisories; Rust's policy can also reject an advisory without a published fix.
There are no advisory-ID allowlists. Resolve a compatible parent update first. Necessary
fork patches remain small, tested and pinned by immutable commit; both server repositories
keep the same engine patch. A local path override is for development only and must never
enter a final product lockfile.

The [Zen fork change](https://github.com/youmssi/zen/pull/1) updates the ZIP reader while
keeping the 2.1.6 evaluation code. The engine drift check compares both servers' resolved
versions and Git source for Zen engine, expression and template crates. The
[tsgo-wasm fork change](https://github.com/youmssi/tsgo-wasm/pull/1) updates Wasmtime and WASI
without changing the TypeScript schema module. The server lockfiles record the exact revisions.

The [editor fork build](https://github.com/youmssi/jdm-editor/releases/tag/v1.52.0-donka.1)
keeps the upstream MIT source and resolves Zod 4 instead of the vulnerable Zod 3 range.
The web lockfile verifies the released tarball's integrity. Return to a compatible upstream
release when it carries the same security fix and passes the editor's live feature checks.

Same-major package overrides must name the cause and be removed when the parent dependency
adopts the patched range. A clean audit describes the advisory database at execution time;
it is not a promise that future advisories cannot appear.

## Container checks before publication

Both product image workflows build an OCI layout, with SBOM and provenance, before publishing
any tag. Trivy scans that layout for **fixable HIGH and CRITICAL** findings and returns a
nonzero status on a match. The action is pinned by commit and uses Trivy 0.76.1. A failed scan
stops publication. The ordinary PR image jobs scan too.

After a successful scan, Skopeo copies the entire OCI index with `--all --preserve-digests`.
The workflow verifies that the registry's index hashes to the build digest. This preserves
the scanned content and its attestation manifests rather than rebuilding after the scan.
Signing and public-key verification follow their existing release policy.

An equivalent local check, after building an OCI directory, is:

```bash
docker buildx build --sbom=true --provenance=mode=max \
  --output type=oci,dest=coverage/image,tar=false .
trivy image --input coverage/image --scanners vuln --severity HIGH,CRITICAL \
  --ignore-unfixed --exit-code 1
```

For Studio, pass `-f apps/app/Dockerfile`. Source dependency audits remain necessary alongside
the container scan, including when a compiled dependency cannot be identified in the image.
These gates do not replace the fresh-install, signing, licence or recovery-rehearsal gates.
