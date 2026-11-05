# ADR-008: Compare Studio performance on matching hardware

- **Status:** accepted
- **Date:** 2026-10-31
- **Story:** DNK-67

## Context

The reference is GitHub's standard 4-vCPU, 16-GB runner. Three runs of the same commit measured
3,000 requests per case, with zero errors and all absolute targets met. GitHub assigned AMD
EPYC 7763, AMD EPYC 9V74 and Intel Xeon Platinum 8573C processors. A shared mean would reject
four unchanged-code cases at the agreed 20% margin. Larger samples do not make different
processor models comparable. The measured values and sources are in [the performance guide](../performance.md).

## Options considered

1. **A baseline per CPU model** — retain standard GitHub runners and compare matching hardware.
   New hardware requires a measured profile before CI can pass.
2. **One fixed reference runner** — control the hardware, with a dedicated runner to provision
   and maintain.
3. **One mean across the assigned models** — would make the agreed gate reject observed normal
   runs. Increasing the margin would change the chosen regression policy.

## Decision

Keep the standard runner and the 20% p95 margin. Studio baselines match CPU model, virtual
core count, architecture and Node major version, as well as sample settings and actual corpus
size. CPU names ignore case and redundant whitespace. Each recorded model uses the pooled
p95 of its own three 1,000-request rounds, not a mean with other hardware models.

Always enforce zero errors and the absolute targets, including individual rounds. CI also
requires one complete comparable profile. An unknown model, different sample, missing metric
or duplicate matching profile fails after the report is saved. Local measurements without a
comparable profile explicitly report that they checked only the absolute targets.

## Consequences

- A CPU assigned by GitHub cannot silently borrow another model's baseline or skip CI's
  regression comparison.
- Adding a model requires reviewing its complete measurement and provenance, recording a
  profile, and validating through CI. Existing values are not raised to conceal a regression.
- Matching a CPU model does not eliminate host-load or scheduling variation. Revisit the
  reference runner if measurements on matching profiles still cannot support the 20% policy.
- This decision changes the measurement gate; application APIs and database modules are
  unaffected. Runtime throughput baselines retain their existing policy (amended below).

## Amendment, 2026-11-04 (DNK-67)

A documentation-only change failed the gate: replay p95 6.13 ms against the matching
4.36 ms profile, while the same code had passed on `develop`. Matching profiles did not
support a pure 20% margin on measures of a few milliseconds. With the owner's agreement, a
case now fails above the larger of its baseline plus 20% and its baseline plus 5 ms
(`margin_floor_ms` in `docs/performance.json`). The margin, the profiles and the absolute
targets are unchanged.

## Amendment, 2026-11-05 (DNK-67)

The Runtime's maximum-throughput baseline was one mean across whichever processors GitHub
assigned. A documentation-only change measured 3,020/s for kyc-risk-rating on an AMD EPYC 7763
against that 3,800/s mean, below its 20% margin, while the model's own seven runs ranged from
3,020 to 3,956/s. With the owner's agreement, the Runtime follows this decision too: one
throughput profile per CPU model, core count and architecture, each the mean of that model's CI
runs (`baseline.runtime.profiles`, runs listed in `source.runs`). In CI, a processor without a
profile fails after its results are saved; the margin and the targets are unchanged.

## Amendment, 2026-11-05: averaged profiles and a 10 ms floor (DNK-67)

A second documentation-only change failed: the newest-records search measured 20.17 ms on an
Intel Xeon 8573C against that model's 15.08 ms profile, taken from a single calibration run,
while the same search measured 19 to 22 ms on every other model and on that model's later runs.
With the owner's agreement, each Studio profile is now the mean of its CPU model's CI runs
(`source.runs`), and the floor is 10 ms: a case fails above the larger of its baseline plus 20%
and its baseline plus 10 ms. Every unchanged-code run recorded so far passes with at least 5% to
spare; the absolute targets are unchanged.
