# Stage 2 — assisted creation, Fieldkit and governance (stories)

Scope and exit gates are in [`docs/delivery-plan.md`](../delivery-plan.md#phase-2--assisted-creation-fieldkit-and-governance).
Stories here are **agreed candidates**: their order, and the Fieldkit and AI-authoring stories,
are set at phase entry. Nothing here starts before the Phase 1 exit gate. Repos: **S** = donka
(Studio), **R** = donka-runtime, **C** = donka-cli. Every story also carries the standing criteria
of Stage 1, including live browser verification.

---

### DNK-56 — See a rule change's impact on past decisions

**Type:** feature · **Repos:** S · **Dependencies:** DNK-18, DNK-20 (merged) · **Size:** L

#### Why
Before approving a new cut-off or points table, a credit owner wants to know how many past
applicants it would have accepted, declined or referred differently. Today replay (DNK-18) runs
one record against the release it ran; nobody can run real history against a draft, so the
impact of a change is guessed from a few test scenarios.

#### Decisions
Confirmed 2026-10-25: a Phase 2 candidate, built on DNK-18 replay rather than a separate
engine; shown where the change is decided (the production approval review) and in the Phase 2
playground next to a proposed diff.

[INTERACTIVE STEP] at story start, with the pilot's volumes in hand:
- the maximum number of records per run and whether runs are queued in the background;
- whether editors may start a run or only owners, since it reads many personal records.

#### Behaviour
| Where | Before | After |
|---|---|---|
| Decision editor / playground | Simulate one input; scenarios | Run the draft or a version against recorded production decisions of a chosen period and decision |
| Production approval review | Diff of versions and contract | The same comparison for the release under review, against what production decided |

#### Acceptance criteria
- [ ] A run picks a decision, a period and a candidate (draft or version) and re-evaluates the matching records server side through `DecisionRuntime`; connector nodes answer from each record's trace and no outside call is made
- [ ] The result shows the number of records compared, the distribution of the outcome field before and after, and an outcome-to-outcome table; each changed cell links to its records
- [ ] Records the candidate cannot compare (a connector the original did not call, an input the new contract rejects, an evaluation error) are counted and listed by reason, never dropped silently
- [ ] Runs are bounded by configuration, cancellable, project-scoped and audited; personal input stays encrypted at rest and redacted where the contract marks it
- [ ] Viewers of another project see nothing; an empty period shows an empty state
- [ ] Strings in English and French; tests cover each criterion above

#### Out of scope
Real repayment performance and default rates (a later candidate, needs outcome labels from the
customer); traffic splitting between versions in the Runtime; statistical scorecard building.

---

### DNK-57 — Watch whether decision inputs drift

**Type:** feature · **Repos:** S · **Dependencies:** DNK-18 (merged) · **Size:** M

#### Why
A scorecard tuned on last year's applicants silently degrades when the population changes:
income bands shift, a channel brings younger applicants. Owners notice only when losses show.
The decision log already holds every production input and outcome, so the shift can be shown
months earlier, without any new data from the customer.

#### Decisions
Confirmed 2026-10-25: a Phase 2 candidate. Stability is measured with the population stability
index (PSI) between a reference period and a recent one, per contract field and on the outcome
field. Thresholds come from configuration; the common 0.1 / 0.25 bands are the documented
defaults, not hard-coded.

[INTERACTIVE STEP] at story start: whether a breach notifies anyone (email through
`crates/mail`) or stays on the screen until Phase 2 webhooks exist.

#### Behaviour
| Where | Before | After |
|---|---|---|
| Project decision log | Search and open records | A stability view per decision: reference vs recent period, PSI per field and for the outcome, status per configured threshold |

#### Acceptance criteria
- [ ] PSI is computed per numeric field (configured bins), per enum field (its values) and for the outcome field, from production records only
- [ ] The reference and recent periods are chosen by the owner; a period with too few records says so instead of showing a value
- [ ] Fields marked personal in the contract show only aggregated bins, never values
- [ ] Computation reads search fields or decrypts server side; nothing decrypted is stored
- [ ] Project isolation and viewer access match the decision log; opening the view is audited
- [ ] Strings in English and French; tests cover each criterion above, including the PSI arithmetic against a worked example

#### Out of scope
Default rate by score band and any model performance metric (needs outcome labels);
automatic rule changes; Runtime-side monitoring.

---

## Later candidates (not agreed)

Kept for the phase-entry discussion; each needs its own story and the owner's agreement.

- **Repayment outcomes and default rate by band.** Import performance labels (e.g. 90 days past
  due) from the core banking system, linked by `X-Donka-Reference`. Needs an ingest API, a
  retention rule and a personal-data decision. Ask the pilot whether labelled history exists.
- **Shadow evaluation, then champion/challenger.** The Runtime evaluates a second release and
  logs it without applying it; a later step splits traffic deterministically by reference.
  Touches the release artifact contract (Runtime ships first), production approvals and
  explanations. Only useful once DNK-56 and DNK-57 exist.
- **Statistical scorecard package**, separate from Studio like Fieldkit: a Python tool for data
  scientists (binning, WoE, logistic regression with open-source libraries such as optbinning)
  that writes a pack in format v1, imported through DNK-43 and released through the normal
  approval workflow. Training data never enters Studio. Not before Fieldkit Stage 1.
