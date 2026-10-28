# Upgrading the decision engine

Studio and Donka Runtime evaluate with one exact zen-engine version (ADR-003). An upgrade is
accepted on evidence, not on the changelog: every pack scenario and a replay of recorded
decisions must give the same results, or the difference is explained to customers. DNK-62
(2.0.1 to 2.1.6) is the reference.

## 1. Read what changed

- Read `CHANGELOG.md` of `zen-engine`, `zen-expression` and `zen-types` in
  `~/.cargo/registry/src/…` after `cargo update`, from the current pin to the target.
- Diff the published sources of the two versions (`diff -r zen-engine-<old> zen-engine-<new>`),
  at least `src/nodes/` (evaluation), `src/nodes/context.rs` (input validation) and
  `zen-types/src/` (serialisation). A changed default, a validation rule or a number format is
  a behaviour change even when the changelog calls it a fix.
- List each behaviour change, then show it with a test or document it for customers.

## 2. Runtime first

In `donka-runtime`, on `dnk-<n>-…` from `develop`:

1. Pin `zen-engine`, `zen-expression` and `zen-tmpl` to the same `=x.y.z` in `Cargo.toml` and
   `crates/connectors/Cargo.toml`; keep `features = ["arbitrary_precision"]`.
2. Run its checks (AGENTS.md §5). A test that changes result is a behaviour change: understand
   it before you touch the test.
3. Add a regression test per behaviour change that reaches a caller (DNK-62:
   `tests/self_reference.rs`, `null` inputs in `tests/contract.rs`). Run it against the old pin
   once to see it fail there.
4. Update `docs/contracts.md` or the page the change concerns, `DONKA.md` and the README badge.
5. Merge before Studio.

## 3. Studio

1. Pin the same version in `Cargo.toml` and `crates/engine/Cargo.toml` (`zen-expression`), and
   move `donka-connectors` to the Runtime's merged `develop` commit.
   `scripts/check-engine-version.sh` must pass.
2. `crates/engine/tests/runtime.rs` shows each behaviour change through `ZenRuntime`.
3. `cargo test --workspace` runs every pack scenario in Studio
   (`every_pack_of_the_catalogue_imports_and_its_scenarios_pass`).
4. `scripts/e2e.sh` releases every catalogue pack to a Runtime, checks each scenario there, and
   replays a logged decision. Run it twice:
   - with the new Runtime (what CI runs);
   - with a Runtime built from the old pin (`RUNTIME_BIN=…`), so records logged by the old
     engine replay identically on the new one.
5. Check `@gorules/jdm-editor` is on its latest release: its bundled `@gorules/zen-engine-wasm`
   drives the expression hints in the editor.
6. Update NOTICE when a new package comes in, ADR-003, the stack in `AGENTS.md`,
   `docs/ARCHITECTURE.md` and the README badge.

## 4. Tell customers

The squash commit's body becomes the release notes: list every behaviour change in words a
customer recognises (what a request gets now that it did not before), not the upstream issue
numbers.
