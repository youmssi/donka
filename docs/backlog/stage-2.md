# Stage 2 — assisted creation, Fieldkit and governance (stories)

Scope and exit gates are in [`docs/delivery-plan.md`](../delivery-plan.md#phase-2--assisted-creation-fieldkit-and-governance).
Stories here are **agreed candidates**: their final order, and the Fieldkit and AI-authoring
stories, are set at phase entry. Nothing here starts before the Phase 1 exit gate. Repos:
**S** = donka (Studio), **R** = donka-runtime, **C** = donka-cli, **K** = donka-scorecard. Every
story also carries the standing criteria of Stage 1, including live browser verification.

Order agreed 2026-10-25 for these candidates: DNK-60, DNK-56, DNK-57, then DNK-58 once the
playground story exists. DNK-61 is a separate repository and runs in parallel. DNK-70 and
DNK-71 are agreed candidates without a fixed slot; DNK-72 is scoped with the Fieldkit story.

---

### DNK-60 — Sign in with the organisation's identity provider

**Type:** feature · **Repos:** S · **Dependencies:** DNK-59 · **Size:** M

#### Why
Banks manage joiners, leavers and their second-factor policy in one directory (Entra ID, AD FS,
Keycloak, Okta…). Separate Studio passwords escape that control and are the first finding of a
security review.

#### Decisions
Confirmed 2026-10-25 (moved from Phase 3): OpenID Connect authorization code flow with PKCE
through `openidconnect` (MIT). Studio stays the place where permissions are decided: the
provider proves who the person is and, optionally, which groups they belong to; Studio maps a
configured group to instance administrator. Project roles stay managed in Studio (global roles
remain Phase 3). No identity provider ships with Donka; CI tests against a Keycloak container,
which is also the documented choice for a customer without one. Local accounts remain for a
break-glass administrator with a second step (DNK-59).

[INTERACTIVE STEP] at story start: create accounts on first sign-in from the provider, or only
for people already invited in Studio.

#### Acceptance criteria
- [ ] Issuer, client, secret, scopes and the group claim come from `DONKA_OIDC_*` configuration; a wrong issuer fails at start
- [ ] Sign-in through the provider opens an ordinary Studio session (same idle timeout, CSRF, sign-out); state, nonce and PKCE are checked
- [ ] A person is matched by the provider's subject, not only by email; an email change at the provider does not create a second account
- [ ] The configured group grants or removes instance administrator at each sign-in; the change is audited
- [ ] Local password sign-in can be limited to named break-glass accounts
- [ ] Provider errors show a generic message and are logged without tokens
- [ ] Strings in English and French; integration tests against Keycloak and a live Playwright journey

#### Out of scope
SAML; mapping provider groups to project roles (Phase 3 global roles); SCIM provisioning.

---

### DNK-56 — See which past applicants a rule change swaps in or out

**Type:** feature · **Repos:** S · **Dependencies:** DNK-18, DNK-15 (merged) · **Size:** L

#### Why
Before approving a new cut-off or points table, a credit owner wants to know which past
applicants it would have accepted that were declined (swap-in), and the reverse (swap-out).
Today replay (DNK-18) runs one record against the release it ran; a change's impact is guessed
from a few test scenarios.

#### Decisions
Confirmed 2026-10-25:
- A new module `crates/analysis`, used only through the public APIs of the decision log and
  decisions, runs **analysis jobs** in the background: queued, bounded, cancellable, with
  progress, one at a time per project. It can be turned off by configuration. DNK-57 reuses it.
- Evaluation goes through `DecisionRuntime`; connector nodes answer from each record's trace
  (DNK-18), so no outside call is made.
- Results keep counts and the identifiers of changed records, never decrypted input. They are
  purged with the records they refer to.
- First shown in the production approval review: the release under review against what
  production decided. The playground view is DNK-58.
- The screen states what the analysis cannot say: past declines have no repayment history, so
  it measures volume and mix, not risk.

[INTERACTIVE STEP] at story start, with the pilot's volumes in hand: the maximum records per
run, and whether editors may start a run or only owners.

#### Behaviour
| Where | Before | After |
|---|---|---|
| Production approval review | Diff of versions and contract | Swap-in/swap-out for a chosen period: outcome mix before and after, and an outcome-to-outcome table |

#### Acceptance criteria
- [ ] An owner starts a run for a decision and period; it reports progress, can be cancelled, and a second run for the project waits
- [ ] The result shows records compared, the outcome mix before and after and the outcome-to-outcome table; each changed cell lists its records
- [ ] Records that cannot be compared (a connector call the original did not make, input the new contract rejects, an evaluation error, a time-dependent expression) are counted by reason, never dropped
- [ ] Runs are bounded by configuration and audited with the number of records read; personal input stays encrypted at rest
- [ ] Another project's members see nothing; an empty period shows an empty state
- [ ] The table scrolls inside its area at 390 px; strings in English and French; tests cover each criterion above

#### Out of scope
Risk impact (needs repayment outcomes, a later candidate); the playground view (DNK-58);
Runtime traffic splitting.

---

### DNK-57 — Watch the decision mix and population stability

**Type:** feature · **Repos:** S · **Dependencies:** DNK-56 · **Size:** M

#### Why
A scorecard tuned on last year's applicants silently degrades when the population changes. The
decision log already holds every production input and output, so the shift can be shown months
before losses do, without new data from the customer.

#### Decisions
Confirmed 2026-10-25:
- First the **decision mix** over time (share of each outcome per week), readable by the business.
- Then stability in the terms risk teams use: **SSI/PSI** on a numeric output the owner picks
  (usually the score), and **CSI** per contract input, between a reference and a recent period.
- Computed as analysis jobs (DNK-56), on demand or on a configured schedule, and kept as dated
  snapshots; nothing decrypted is stored.
- Thresholds come from configuration; 0.1 and 0.25 are the documented defaults.
- Bins with fewer than a configured number of records are hidden, so a person cannot be
  singled out.

[INTERACTIVE STEP] at story start: whether a breach emails the owners or stays on the screen
until webhooks exist.

#### Acceptance criteria
- [ ] The decision mix shows each outcome's share per week for a chosen period
- [ ] SSI on the chosen output and CSI per input use configured bins for numbers and the values for enums; a period with too few records says so instead of showing a value
- [ ] Small bins are suppressed; personal fields show only aggregated bins
- [ ] Each snapshot records its periods, bins and thresholds; project isolation matches the decision log; viewing is audited
- [ ] Strings in English and French; tests cover each criterion above, including the PSI arithmetic against a worked example

#### Out of scope
Default rate, Gini or KS in production (need repayment outcomes); automatic rule changes.

---

### DNK-58 — Swap-in and swap-out in the playground

**Type:** feature · **Repos:** S · **Dependencies:** DNK-56 and the Phase 2 playground story · **Size:** S

#### Why
An owner reviewing a proposed diff in the playground should see its impact on real past
decisions before accepting it, not only after a release is requested.

#### Acceptance criteria
- [ ] A proposed or edited draft can be run against recorded decisions with the DNK-56 job, from the playground
- [ ] The result sits next to the diff, with the same limits, audit and wording as DNK-56
- [ ] Strings in English and French; tests cover each criterion above

---

### DNK-61 — Build a statistical scorecard as a Donka pack

**Type:** feature · **Repos:** K (new, open source) · **Dependencies:** DNK-43 (merged) · **Size:** L

#### Why
Lenders with enough repayment history want a statistical scorecard rather than expert points,
and their data scientists work in Python. Without a bridge, the model is rebuilt by hand in
Studio and nobody can prove the two give the same score.

#### Decisions
Confirmed 2026-10-25:
- Separate repository `donka-scorecard`, **Apache-2.0**, Python 3.11+. Training data never
  enters Studio; the only link is pack format v1.
- One binning engine: **optbinning** (Apache-2.0), pinned and behind a thin internal layer.
  scorecardpy and toad are not dependencies; an independent WoE/IV computation in the tests
  checks the arithmetic.
- A versioned configuration file drives a run: target definition, observation and
  performance windows, characteristics, monotonic and minimum-bin constraints, special values
  and the points scaling (base points, odds, points to double the odds).
- Validation is reported, not assumed: out-of-time sample, Gini, KS and stability.

#### Acceptance criteria
- [ ] `donka-scorecard build <config>` writes a pack in format v1: points tables per characteristic (missing and special bins included), the score, bands and decision, the input contract, guides EN/FR
- [ ] The pack's scenarios are real sample rows with the score computed in Python; importing the pack in Studio runs them and they all pass
- [ ] A model report states the target, windows, sample sizes, bins, WoE/IV, out-of-time Gini/KS and the known limits (accepted-only bias, small samples)
- [ ] A run with too few bad outcomes for the configured minimum stops with a clear message
- [ ] The same configuration and data give the same pack
- [ ] Tests, CI, licence and contribution files are in place; the README explains the Studio import

#### Out of scope
Reject inference (documented as a limitation); machine-learning models other than the
scorecard; uploading to Studio directly (import through DNK-43).

---

### DNK-70 — Evaluate a portfolio in one request

**Type:** feature · **Repos:** R, S · **Dependencies:** DNK-18 (merged) · **Size:** M

#### Why
Banks re-score whole portfolios: monthly limit reviews, campaigns, a policy change applied to
existing customers. Calling the Runtime once per customer from a script is slow and leaves no
grouped trace.

#### Decisions
Confirmed 2026-10-25 as a Phase 2 candidate: a batch endpoint on the Runtime evaluating many
inputs against one release, each logged as an ordinary record with a shared batch reference.

#### Acceptance criteria
- [ ] One request evaluates up to a configured number of inputs and returns each result or error in order
- [ ] Each evaluation is recorded in the decision log with its own reference and the batch's
- [ ] Contract validation, connectors, rate limits and tokens apply per evaluation as for single calls
- [ ] Studio's decision log filters by batch; tests cover limits, partial failures and isolation

---

### DNK-71 — Keep protected characteristics out of decisions

**Type:** feature · **Repos:** S · **Dependencies:** DNK-37 (merged) · **Size:** S

#### Why
Fair-lending rules forbid deciding on characteristics such as sex, religion or ethnicity, and
supervisors increasingly ask lenders to show it. Today nothing stops a rule from reading them.

#### Decisions
Confirmed 2026-10-25 as a Phase 2 candidate: the input contract marks a field as protected,
next to the existing personal-data flag. Using a protected field in a rule warns at save and
blocks the production approval unless an owner records a justification.

#### Acceptance criteria
- [ ] Contract fields can be marked protected; the mark is visible in the editor and the contract diff
- [ ] Saving a decision that reads a protected field warns, naming the node and field
- [ ] Requesting production for such a release requires a recorded justification, shown in the approval and the audit log
- [ ] Packs ship with their protected fields marked; tests cover each rule above

---

### Fieldkit baseline (input to the Phase 2 Fieldkit story)

Agreed 2026-10-25: field-collection tools such as KoboToolbox and ODK set the expected baseline.
The Fieldkit story is written to meet it, then to add what only Fieldkit does:

- offline first, with synchronisation, retries and conflict handling when two agents edit
  the same application;
- GPS location, photos and signatures captured with the form;
- data encrypted on the device, wiped on sign-out or remote revocation;
- form versions, with submissions kept against the version that produced them;
- English and French forms from the contract's labels;
- agents, teams and devices managed by an administrator;
- **the decision on the device**: the published release evaluated locally through the engine's
  WebAssembly build, then confirmed by the Runtime on synchronisation.

---

### DNK-72 — Fieldkit on its own, and with XLSForm

**Type:** feature · **Repos:** Fieldkit · **Dependencies:** the Phase 2 Fieldkit story · **Size:** M

#### Why
Field teams in Africa largely collect with KoboToolbox and ODK, which use XLSForm. Fieldkit
reaches beyond Donka customers if it runs without Studio and speaks that format.

#### Decisions
Confirmed 2026-10-25 as a Phase 2 candidate, its scope set with the Fieldkit story at phase
entry: a standalone mode (forms from a JSON Schema contract, submissions to a webhook) and
XLSForm import and export.

#### Acceptance criteria
- [ ] Fieldkit renders and validates a form from a JSON Schema contract with no Studio, and posts submissions to a configured webhook
- [ ] An XLSForm imports into a Fieldkit form, and a Fieldkit form exports to XLSForm; unsupported constructs are listed, never dropped silently
- [ ] With Studio, the same form still decides through the published release

---

## Later candidates (not agreed)

Kept for the phase-entry discussion; each needs its own story and the owner's agreement.

- **Repayment outcomes.** Import performance labels (e.g. 90 days past due) from the core
  banking system, linked by `X-Donka-Reference`; then default rate by band, Gini and KS in
  production. Needs an ingest API, retention and a personal-data decision. Name it
  "repayment performance": in Donka, "outcome" is the decision's own output field. Ask the pilot
  whether labelled history exists.
- **Shadow evaluation, then champion/challenger.** The Runtime evaluates a second release and
  logs it without applying it; a later step splits traffic deterministically by reference.
  Touches the release artifact contract (Runtime ships first), approvals and explanations.
- **Passkeys** (`webauthn-rs`) for local accounts, after DNK-59.
- **Fieldkit Cloud**: forms and submissions as a hosted service for small institutions, with
  the renderer open source; decided after the pilot shows whether field agents use Fieldkit.
