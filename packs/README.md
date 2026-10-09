# Starter packs

Ready-made projects to start from instead of an empty canvas. Each pack holds decisions, test
scenarios that pass and a short guide in English and French. Importing one creates an ordinary
project: change anything, nothing links it back to the pack, and a newer version of the pack
never overwrites it. Import the same pack again under another key to start a second, independent
product from it.

| Pack | What it decides | Guide |
| --- | --- | --- |
| [retail-credit](retail-credit) | Personal loans in Cameroon (CEMAC, XAF): affordability, scorecard, bands, reason codes | [EN](retail-credit/guide.md) · [FR](retail-credit/guide.fr.md) |
| [sme-treasury](sme-treasury) | Overdrafts and short-term loans for small businesses (CEMAC, XAF): capacity by sector, cash-flow cover | [EN](sme-treasury/guide.md) · [FR](sme-treasury/guide.fr.md) |
| [kyc-risk-rating](kyc-risk-rating) | Money-laundering risk of a customer (CEMAC): rating, due-diligence level, next review | [EN](kyc-risk-rating/guide.md) · [FR](kyc-risk-rating/guide.fr.md) |
| [mobile-money-limits](mobile-money-limits) | Mobile money transactions against the wallet's KYC tier (CEMAC, XAF): limits, fees, upgrade | [EN](mobile-money-limits/guide.md) · [FR](mobile-money-limits/guide.fr.md) |

## Import a pack

Studio offers the packs of the folder `DONKA_PACKS_DIR` names; the container image ships these
four in `/srv/packs`. An administrator imports one from **Projects → From a pack**, under a name
and key of their choice. The **Pack file** tab imports a pack as a `.zip` instead, for example one
exported from another installation.

The same from a script, with an administrator's account (needs curl, jq and zip):

```bash
STUDIO_URL=https://donka.bank.example DONKA_EMAIL=you@bank.example DONKA_PASSWORD='...' \
scripts/import-pack.sh packs/retail-credit
# A second copy for another product:
PROJECT_KEY=salary-advance PROJECT_NAME="Salary advance" scripts/import-pack.sh packs/retail-credit
```

## Duplicate or export a project

In a project's **Settings**, **Copy or export** starts from the current drafts or from a release:

- **Duplicate** (administrators) creates a new project with the same decisions, test scenarios,
  input contracts and decision-log settings. The person duplicating is its only owner.
- **Export pack file** (owners) downloads `<key>.donka-pack.zip`, a pack in the format below that
  another team or installation imports.

Neither carries releases, deployments, tokens, decision records or members. The new project's
audit log opens with where it came from; the source project's log records the copy or the export.

## Format (version 1)

```
packs/<key>/
  pack.json        format, key, name and description (en, fr), version, market, currency,
                   entry decision, decisions in dependency order, decision-log settings
  decisions/       one JDM graph per decision, named by its key; an entry decision carries its
                   input contract (JSON Schema, DNK-37) on its Request node
  scenarios.json   test scenarios: decision key, name, input, expected, match
  guide.md         what it reads and answers, the policy to confirm, how to adapt it
  guide.fr.md
```

A pack file is a zip of the same layout, at its root or inside one folder, at most 5 MB.
Studio checks a pack in full before it creates anything: every decision is a graph the engine
accepts with a usable contract, a decision comes after those it calls, scenarios name a decision
of the pack and two scenarios of a decision have different names. A pack that fails says why.

CI imports every pack and checks all its scenarios pass (`apps/app/tests/packs.rs`), and that
each contract matches what its rules read (`crates/pack`).
