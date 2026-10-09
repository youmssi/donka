# Retail credit (Cameroon, XAF)

[Français](guide.fr.md)

A personal-loan scorecard to start from: it checks affordability, scores the applicant, places
them in a band and answers **approve**, **refer** (an analyst decides) or **decline**, with reason
codes in English and French. Amounts are in FCFA (XAF), without decimals.

> **A starting point, not a validated model.** The points, bands and limits are expert defaults
> chosen to be plausible for a Cameroonian lender. Before production, your risk team sets them
> to your credit policy and checks the bands against your own portfolio. Every value to review
> is in the **Credit policy** node or in a points table.

## Import it

```bash
STUDIO_URL=https://donka.bank.example DONKA_EMAIL=you@bank.example DONKA_PASSWORD='...' \
scripts/import-pack.sh packs/retail-credit
```

This creates the project `retail-credit` with two decisions, a first version of each and 8 test
scenarios, all passing. It is your project: change anything. To start another product from the
same pack, import it again under another key:
`PROJECT_KEY=salary-advance PROJECT_NAME="Salary advance" scripts/import-pack.sh packs/retail-credit`.

## What it reads

| Field | Type | Meaning |
| --- | --- | --- |
| `applicant.age` | number | Age in years |
| `applicant.employment` | `public`, `private`, `self-employed`, `informal` | Type of income |
| `applicant.monthsInJob` | number | Months with the current employer or activity |
| `applicant.monthlyIncome` | number (XAF) | Net monthly income |
| `applicant.monthlyDebtPayments` | number (XAF) | Repayments on loans already running |
| `applicant.salaryDomiciled` | boolean | The salary is paid into an account at this bank |
| `loan.amount` | number (XAF) | Amount asked for |
| `loan.termMonths` | number | Duration in months |
| `bureau.checked` | boolean | A credit bureau was consulted for this application |
| `bureau.activeLoans` | number | Loans currently running elsewhere |
| `bureau.worstDaysPastDue` | number | Worst arrears reported, in days (24 months) |

These fields are the scorecard's input contract (**Input fields** in the editor): the Runtime refuses a
request that lacks one or breaks its limits with `400`, naming the field, and forms can be built
from the same definition.

The `bureau` fields are filled by your system from the source you consult (a private credit
bureau or the central bank's risk register). If you call the bureau from the Runtime instead,
replace them with a connector node (see the Runtime's connectors guide).

## What it answers

```json
{
  "decision": "approve",
  "band": "A",
  "score": 143,
  "reasons": [],
  "installment": 87893,
  "debtToIncome": 0.284,
  "maxAmount": 3980000,
  "points": { "age": 25, "employment": 30, "tenure": 18, "debtToIncome": 20, "history": 30, "salaryDomiciled": 20 }
}
```

`reasons` lists knock-out codes (`K..`) first, then reason codes (`R..`), each with `code`, `en`
and `fr`. `maxAmount` is the largest loan the applicant can afford at this term.

## How it decides

1. **Credit policy**: the values your risk team owns (below).
2. **Affordability** (its own decision, also usable alone): monthly instalment at the policy rate,
   debt-to-income ratio after the new loan, and the largest affordable amount.
3. **Points**: age, employment type, time in job, debt-to-income, credit history and salary
   domiciliation, one decision table each (160 points at most).
4. **Band**: A ≥ 120 and B ≥ 100 approve, C ≥ 80 refers, D and E decline.
5. **Knock-out rules**: any one of them declines, whatever the score.
6. **Decision**: a knock-out declines; without a bureau check, an approval becomes a referral.

### Credit policy (to confirm)

| Setting | Default | Meaning |
| --- | --- | --- |
| `minAge` | 21 | Youngest applicant |
| `maxAgeAtMaturity` | 65 | Oldest age at the last instalment |
| `minMonthlyIncome` | 75,000 | Lowest net monthly income (XAF) |
| `maxDebtToIncome` | 0.40 | Highest share of income going to repayments |
| `maxDaysPastDue` | 90 | Arrears beyond this decline |
| `maxTermMonths` | 60 | Longest duration |
| `maxAmount` | 10,000,000 | Largest amount (XAF) |
| `annualRate` | 0.16 | Nominal yearly rate used for the instalment; keep it within the legal maximum |

### Knock-out and reason codes

| Code | Meaning |
| --- | --- |
| K01 | Below the minimum age |
| K02 | The loan would end after the maximum age |
| K03 | Income below the minimum |
| K04 | Repayments above the debt-to-income limit |
| K05 | Serious arrears at the bureau |
| K06 | Term above the maximum |
| K07 | Amount above the maximum |
| R10 | High debt-to-income ratio |
| R20 | Late payments in the last 24 months |
| R21 | Several loans already active |
| R30 | Less than a year in the current job |
| R40 | Self-employed or informal income |
| R50 | Salary not paid into this bank |
| R60 | No bureau check: an analyst must review |

## Adapt it

- Change a policy value in the **Credit policy** node, save a version: the 8 scenarios run and
  show what your change moves.
- Add a characteristic: add a points table, add its points to the **Score** node, and widen the
  bands.
- Before going live, run a sample of past applications through the simulator and compare the
  bands with what happened to those loans.
