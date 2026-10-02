# Decision-log feed

The feed is how Donka Runtime sends Studio a record of every decision it makes (DNK-18). Like
the release artifact (`artifact-format.md`), it is a contract between the two: changes are
**additive only**, and the Runtime ships first when it changes.

## Request

```
POST /api/v1/decision-log/records
Authorization: Bearer dnk_log_…
Content-Type: application/json

{ "records": [ { … }, … ] }
```

- The token is a **decision-log token**, issued by a Studio administrator under **Runtimes**
  for one environment (staging or production), shown once, kept only as a SHA-256. The Runtime
  reads it from `DECISION_LOG__TOKEN` and the address from `DECISION_LOG__URL`.
- No session cookie and no CSRF header: the bearer token is the only credential.
- At most **1000 records** per request (the Runtime sends 100 by default) and 16 MiB.

Each record:

| Field | Type | Meaning |
|---|---|---|
| `id` | uuid | Given by the Runtime and returned to the caller as `X-Decision-Id`. A record sent twice (a retry) is stored once. |
| `projectId` | uuid | `project.id` of the artifact that answered. |
| `releaseId` | uuid | `release.id` of the artifact that answered. |
| `environment` | `staging` \| `production` | `environment.key` of the artifact. |
| `decisionKey` | string, 1–200 | The decision evaluated (`person-score`, `bureau/normalize`). |
| `reference` | string, 1–200, optional | The caller's `X-Donka-Reference` (an application or customer number). |
| `evaluatedAt` | ISO-8601 | When the evaluation started. |
| `durationUs` | integer | How long it took, in microseconds. |
| `status` | `succeeded` \| `failed` | |
| `input` | JSON | The context the decision was given. |
| `output` | JSON | The result, when `succeeded`. |
| `error` | JSON | The error body the caller received, when `failed`. |
| `trace` | JSON, optional | The engine's per-node trace (`{ "<node id>": { input, output, traceData, … } }`); up to the failing node for a node error. Connector traces never hold a secret. |

## Answer

`200` with what became of each record:

```json
{ "accepted": 98, "rejected": [ { "id": "…", "code": "unknown_release" } ] }
```

| Code | Why a record is not stored |
|---|---|
| `invalid` | Not a record in this format (a missing field, an empty key, `succeeded` without `output`…). |
| `wrong_environment` | Its environment is not the one the token was issued for. |
| `unknown_release` | Studio never published that release to that environment of that project. |

Records without a readable `id` are skipped and logged by Studio. Other answers:

| Status | Code | The Runtime… |
|---|---|---|
| `400` | `INVALID_REQUEST`, `TOO_MANY_RECORDS` | logs it and drops the batch. |
| `401` | `INVALID_TOKEN` (missing, unknown or revoked token) | logs it and drops the batch. |
| `408`, `429`, `5xx`, no answer | | keeps the batch and tries again (1 s, doubling, at most 30 s). |

## In Studio

- Searchable fields (decision, reference, outcome, environment, status, time) are stored as
  they are. `input`, `output`, `error` and `trace` are encrypted together with AES-256-GCM under
  `DONKA_DECISION_LOG_KEY`, the record id as associated data.
- The **outcome** is read when the record arrives: `error` for a failed evaluation, otherwise the
  value at the project's outcome field (a dotted path such as `decision` or `result.band`) when
  it is a string, number or boolean.
- Records cannot be changed. They are deleted only by the retention purge
  (`DONKA_DECISION_LOG_RETENTION_DAYS`, five years by default), which records one audit event
  per project it purges.
- Any project member can search the log; opening or replaying a record is audited. Replay
  evaluates the record's input with its release, connector nodes answering with their output
  in the record's trace, so no outside service is called.
