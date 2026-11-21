# Mobile money tiered limits (CEMAC, XAF)

[Français](guide.fr.md)

Allows or declines a mobile money transaction against the limits of the wallet's KYC tier:
maximum balance, per transaction, daily and monthly outgoing, and transfers to another CEMAC
country. It answers **allow** or **decline** with the fee, the balance afterwards, what remains of
the daily and monthly limits, reason codes in English and French, and the tier that would allow a
declined transaction. Amounts are in FCFA (XAF).

> **A starting point, not a validated model.** The limits and fees are illustrative defaults. Set
> them to your licence, the BEAC rules on payment services and your own price list before
> production. Every value to review is in the **Limits by tier** and **Fees** tables.

## Import it

In Studio, **Projects → From a pack → Mobile money tiered limits**, then choose the project's
name and key. This creates a project with the decision `transaction-limits`, a first version and
9 test scenarios, all passing.

## What it reads

| Field | Type | Meaning |
| --- | --- | --- |
| `wallet.tier` | 1, 2 or 3 | KYC tier: 1 phone number and declared identity, 2 ID document checked, 3 full KYC |
| `wallet.balance` | number (XAF) | Balance before the transaction |
| `wallet.dailyOutgoing` | number (XAF) | Already sent or withdrawn today |
| `wallet.monthlyOutgoing` | number (XAF) | Already sent or withdrawn this month |
| `transaction.type` | `deposit`, `transfer`, `withdrawal`, `merchant-payment` | What is asked |
| `transaction.amount` | number (XAF) | Amount |
| `transaction.crossBorder` | boolean | To another CEMAC country |

## What it answers

```json
{
  "decision": "decline",
  "fee": 0,
  "balanceAfter": 450000,
  "dailyRemaining": 300000,
  "monthlyRemaining": 1000000,
  "upgradeTo": 2,
  "reasons": [
    { "code": "M01", "en": "Above the tier's limit per transaction.", "fr": "Au-dessus du plafond par opération du niveau." }
  ]
}
```

A declined transaction keeps the balance and the limits as they were. `upgradeTo` is there only
when a higher tier would allow it: never for a balance too low.

## How it decides

1. **Limits by tier** and **Fees** for the transaction.
2. **After the transaction**: balance, and daily and monthly outgoing, as they would be.
3. **Limit checks**: every limit it would break gives a reason. Deposits only meet the maximum
   balance; transfers, withdrawals and payments meet the others.
4. **Decision**: allow when no limit is broken.

### Limits and fees (to confirm)

| Tier | Maximum balance | Per transaction | Daily outgoing | Monthly outgoing | Cross-border |
| --- | --- | --- | --- | --- | --- |
| 1 | 500,000 | 200,000 | 300,000 | 1,000,000 | no |
| 2 | 2,000,000 | 1,000,000 | 1,500,000 | 5,000,000 | yes |
| 3 | 10,000,000 | 5,000,000 | 7,500,000 | 25,000,000 | yes |

| Amount up to | Transfer | Withdrawal |
| --- | --- | --- |
| 5,000 | 50 | 100 |
| 50,000 | 250 | 500 |
| 300,000 | 1,000 | 1,500 |
| more | 2,500 | 3,500 |

Deposits and merchant payments are free for the customer.

### Reason codes

| Code | Meaning |
| --- | --- |
| M01 | Above the tier's limit per transaction |
| M02 | Above the tier's daily limit |
| M03 | Above the tier's monthly limit |
| M04 | The balance does not cover the amount and the fee |
| M05 | The balance would exceed the tier's maximum |
| M06 | Transfers to another CEMAC country need tier 2 or more |

## Adapt it

- Add a row to **Limits by tier** for an agent or merchant wallet, with its own tier number.
- Change a value, save a version: the 9 scenarios show what moves.
