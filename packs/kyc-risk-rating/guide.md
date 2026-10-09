# KYC risk rating (CEMAC)

[Français](guide.fr.md)

Rates the money-laundering and terrorist-financing risk of a customer, at onboarding and at each
review: **low**, **medium**, **high** or **prohibited**. It gives the due-diligence level
(simplified, standard, enhanced), the months until the next review, and answers **accept**,
**refer** (compliance decides) or **reject**, with reason codes in English and French.

> **A starting point, not a validated model.** Points, thresholds and the country lists are
> expert defaults. Your compliance officer sets them to the CEMAC AML/CFT rules, COBAC's
> instructions and your own risk assessment before production. Every value to review is in the
> **AML policy** node or a points table.

## Import it

In Studio, **Projects → From a pack → KYC risk rating**, then choose the project's name and key.
This creates a project with the decision `risk-rating`, a first version and 10 test scenarios,
all passing. Import it again under another key to start another product from it.

## What it reads

| Field | Type | Meaning |
| --- | --- | --- |
| `customer.type` | `individual` or `business` | Who the customer is |
| `customer.residence` | ISO country code | Country of residence |
| `customer.nationality` | ISO country code | Nationality, or country of registration of a business |
| `customer.activity` | `salaried`, `civil-servant`, `self-employed`, `trader`, `ngo`, `cash-intensive`, `other` | Occupation or business activity |
| `customer.pep` | boolean | Politically exposed person, a family member or a close associate |
| `customer.yearsKnown` | number | Years as a customer (0 at onboarding) |
| `customer.beneficialOwnersIdentified` | boolean, businesses | Everyone owning or controlling 25% or more is identified |
| `account.channel` | `branch`, `agent` or `online` | How the relationship started |
| `account.expectedMonthlyVolume` | number (XAF) | Expected monthly volume |
| `screening.sanctionsHit` | boolean | A confirmed match on a sanctions list |
| `screening.adverseMedia` | boolean | Adverse media found |
| `documents.identityVerified` | boolean | Identity document verified |
| `documents.addressVerified` | boolean | Address verified |

These fields are the decision's input contract (**Input fields** in the editor): the Runtime
refuses a request that lacks one with `400`, naming the field.

## What it answers

```json
{
  "decision": "refer",
  "rating": "high",
  "dueDiligence": "enhanced",
  "reviewMonths": 12,
  "score": 60,
  "reasons": [
    { "code": "R04", "en": "Cash-intensive activity.", "fr": "Activité à forte intensité d'espèces." },
    { "code": "R06", "en": "Onboarded without meeting the customer.", "fr": "Entrée en relation à distance." },
    { "code": "R07", "en": "High expected volume.", "fr": "Volume attendu élevé." }
  ],
  "points": { "activity": 30, "geography": 0, "pep": 0, "channel": 10, "volume": 20, "tenure": 0, "media": 0 }
}
```

## How it decides

1. **Geography**: residence or nationality in a high-risk country, or residence outside the CEMAC.
2. **Points**: activity, geography, PEP, channel, expected volume, a new relationship and adverse
   media add up to the score.
3. **Rating**: a sanctions match is prohibited; a PEP or a high-risk country is always high;
   otherwise high from 60 points, medium from 30, low below.
4. **Due diligence and review**: low is simplified and reviewed after 36 months, medium standard
   after 24, high enhanced after 12.
5. **Decision**: prohibited rejects; high, missing beneficial owners or an unverified document
   refers to compliance; anything else accepts.

### AML policy (to confirm)

| Setting | Default | Meaning |
| --- | --- | --- |
| `cemac` | CM, CF, CG, GA, GQ, TD | Countries treated as home market |
| `highRiskCountries` | IR, KP, MM | Countries under a FATF call for action; keep it in step with the FATF lists |
| `highVolume` | 50,000,000 | Expected monthly volume that counts as high |
| `mediumVolume` | 10,000,000 | Expected monthly volume that counts as medium |
| `mediumFrom` | 30 | Score from which the rating is medium |
| `highFrom` | 60 | Score from which the rating is high |

| Factor | Points |
| --- | --- |
| Activity: cash-intensive / NGO / trader / self-employed or other | 30 / 20 / 15 / 10 |
| High-risk country / residence outside the CEMAC | 40 / 15 |
| PEP | 40 |
| Channel: online / agent | 10 / 5 |
| Expected volume: high / medium | 20 / 10 |
| Customer for less than a year | 10 |
| Adverse media | 30 |

### Reason codes

| Code | Meaning |
| --- | --- |
| K01 | Confirmed sanctions match: the relationship cannot proceed |
| R01 | Politically exposed person: enhanced due diligence and senior management approval |
| R02 | Resident or national of a high-risk country |
| R03 | Resident outside the CEMAC |
| R04 | Cash-intensive activity |
| R05 | Adverse media |
| R06 | Onboarded without meeting the customer |
| R07 | High expected volume |
| R08 | Identity document not verified |
| R09 | Address not verified |
| R10 | Beneficial owners not identified |

## Adapt it

- Keep the country lists in step with the FATF statements and your national risk assessment.
- Sanctions screening itself happens before this decision: it reads the result.
- Change a value, save a version: the 10 scenarios show what moves.
