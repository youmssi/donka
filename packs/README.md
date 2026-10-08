# Starter packs

Ready-made projects to start from instead of an empty canvas. Each pack holds decisions, test
scenarios that pass and a short guide in English and French. Importing one creates an ordinary
project: change anything, nothing links it back to the pack. Import the same pack again under
another key to start a second, independent product from it.

| Pack | What it decides | Guide |
| --- | --- | --- |
| [retail-credit](retail-credit) | Personal loans in Cameroon (CEMAC, XAF): affordability, scorecard, bands, reason codes | [EN](retail-credit/guide.md) · [FR](retail-credit/guide.fr.md) |
| [sme-treasury](sme-treasury) | Overdrafts and short-term loans for small businesses (CEMAC, XAF): capacity by sector, cash-flow cover | [EN](sme-treasury/guide.md) · [FR](sme-treasury/guide.fr.md) |

```bash
STUDIO_URL=https://donka.bank.example DONKA_EMAIL=you@bank.example DONKA_PASSWORD='...' \
scripts/import-pack.sh packs/retail-credit
# A second copy for another product:
PROJECT_KEY=salary-advance PROJECT_NAME="Salary advance" scripts/import-pack.sh packs/retail-credit
```

## Format (version 1)

```
packs/<key>/
  pack.json        key, name and description (en, fr), market, currency, entry decision,
                   decisions in dependency order, decision-log settings
  decisions/       one JDM graph per decision, named by its key
  scenarios.json   test scenarios: decision key, name, input, expected, match
  guide.md         what it reads and answers, the policy to confirm, how to adapt it
  guide.fr.md
```

CI imports every pack and checks all its scenarios pass (`apps/app/tests/packs.rs`). Import from
the web app, duplicating a project and exporting one as a pack are planned in DNK-43.
