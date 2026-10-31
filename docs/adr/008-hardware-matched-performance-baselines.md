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
  unaffected. Runtime throughput baselines retain their existing policy.
