# SME treasury evaluation (Cameroon, XAF)

[Français](guide.fr.md)

Evaluates an overdraft or a short-term loan for a small business: what its turnover can carry for
its sector, how well its cash flow covers the repayments, and its track record. It answers
**approve**, **refer** (an analyst decides) or **decline**, with a recommended limit and reason
codes in English and French. Amounts are in FCFA (XAF).

> **A starting point, not a validated model.** Sector factors, points, bands and limits are expert
> defaults. Your risk team sets them to your credit policy and checks them against your own SME
> portfolio before production. Every value to review is in the **Credit policy** node, the
> **Sector factors** table or a points table.

## Import it

```bash
STUDIO_URL=https://donka.bank.example DONKA_EMAIL=you@bank.example DONKA_PASSWORD='...' \
scripts/import-pack.sh packs/sme-treasury
```

This creates the project `sme-treasury` with the decision `evaluation`, a first version and 6
test scenarios, all passing. Import it again under another key
(`PROJECT_KEY=... PROJECT_NAME=...`) to start another product from it.

## What it reads

| Field | Type | Meaning |
| --- | --- | --- |
| `business.sector` | `trade`, `services`, `manufacturing`, `agriculture`, `construction` (others: default factors) | Main activity |
| `business.yearsActive` | number | Years in business |
| `business.monthlyTurnover` | number (XAF) | Average monthly turnover over the last 6 months |
| `business.turnoverTrend` | number | Turnover of the last 3 months ÷ the 3 before (1.1 = +10%) |
| `business.averageBalance` | number (XAF) | Average account balance over the last 6 months |
| `business.monthlyDebtService` | number (XAF) | Repayments on credit already running |
| `business.taxRegistered` | boolean | A tax ID (NIU) is on file |
| `request.product` | `overdraft` or `short-term-loan` | What is asked for |
| `request.amount` | number (XAF) | Limit or amount asked for |
| `request.termMonths` | number | Duration of a short-term loan |
| `bureau.checked` | boolean | A credit bureau was consulted |
| `bureau.paymentIncidents` | number | Payment incidents (e.g. returned cheques), 12 months |
| `bureau.worstDaysPastDue` | number | Worst arrears reported, in days |

These fields are the evaluation's input contract (**Input fields** in the editor): the Runtime
refuses a request that lacks one or breaks its limits with `400`, naming the field.

## What it answers

```json
{
  "decision": "approve",
  "band": "A",
  "score": 130,
  "recommendedLimit": 2000000,
  "capacityLimit": 2000000,
  "debtServiceCover": 2.14,
  "reasons": [
    { "code": "R71", "en": "The request is above the business's capacity: the limit is reduced.", "fr": "La demande dépasse la capacité de l'entreprise : la limite est réduite." }
  ],
  "points": { "years": 25, "trend": 25, "debtServiceCover": 30, "balanceCover": 20, "taxRegistered": 10, "history": 20 }
}
```

`recommendedLimit` is the lower of the amount asked and the business's capacity, and 0 when the
answer is decline.

## How it decides

1. **Credit policy** and **Sector factors**: what share of monthly turnover can become a limit,
   and the cash margin of each sector.
2. **New debt service**: interest on an overdraft, or the instalment of a loan, at the policy rate.
3. **Cover**: debt service cover (monthly cash flow ÷ all repayments) and balance cover (average
   balance ÷ all repayments).
4. **Points**: years active, turnover trend, debt service cover, balance cover, tax ID and credit
   history (130 points at most).
5. **Band**: A ≥ 100 and B ≥ 80 approve, C ≥ 60 refers, D declines.
6. **Decision**: a knock-out declines; an approval without a bureau check, or with debt service
   cover below the policy minimum, becomes a referral.

### Credit policy (to confirm)

| Setting | Default | Meaning |
| --- | --- | --- |
| `minYearsActive` | 2 | Youngest business |
| `minDebtServiceCover` | 1.2 | Below it, an approval goes to an analyst |
| `maxDaysPastDue` | 90 | Arrears beyond this decline |
| `maxPaymentIncidents` | 2 | More incidents decline |
| `annualRate` | 0.14 | Nominal yearly rate for the debt service |

| Sector | Limit share of turnover | Cash margin |
| --- | --- | --- |
| trade | 30% | 10% |
| services | 25% | 20% |
| manufacturing | 25% | 15% |
| agriculture | 20% | 15% |
| construction | 20% | 12% |
| other | 15% | 10% |

### Knock-out and reason codes

| Code | Meaning |
| --- | --- |
| K11 | Business younger than the minimum |
| K12 | No turnover on record |
| K13 | Cash flow does not cover the repayments (cover below 1) |
| K14 | Serious arrears at the bureau |
| K15 | Too many payment incidents |
| R60 | No bureau check: an analyst must review |
| R61 | Turnover is falling |
| R62 | Repayment cover below the policy minimum: an analyst must review |
| R63 | Low average balance |
| R64 | No tax ID (NIU) on file |
| R65 | Late payments in the last 24 months |
| R66 | Young business |
| R71 | Request above capacity: the limit is reduced |

## Adapt it

- Set the sector factors from your own SME book: seasonal activities (agriculture) usually need
  a lower share or a seasonal check.
- Change a value, save a version: the 6 scenarios show what moves.
