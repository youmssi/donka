# Performance

[Français](performance.fr.md)

What the Runtime and the decision-log feed sustain, how it was measured, and how to measure it
on your own servers. The targets and the baseline are in [`performance.json`](performance.json);
CI checks every pull request against them.

## Targets

| Measure | Target |
| --- | --- |
| Peak load | 100 decisions per second, 100,000 per day |
| Runtime latency at 100/s, decision without connector | p95 ≤ 20 ms, p99 ≤ 50 ms, no error |
| Decision-log feed | every decision answered at 100/s is in Studio's log within 5 s |
| Studio stopped, within the Runtime's queue | no decision fails, no record lost |
| Studio stopped, beyond the queue | no decision fails; dropped records are counted in the Runtime's log |
| Studio decision-log search and replay | p95 1 s and 2 s on the documented sample |
| Regression | Runtime maximum throughput more than 20% below baseline, or Studio pooled p95 more than 20% above baseline, fails CI |

The latency is measured from when each request was due, not from when it left, so a slow
Runtime cannot hide its own queueing.

## Results

Reference machine: a GitHub `ubuntu-latest` runner (4 vCPU, 16 GB), Studio, the Runtime,
PostgreSQL and the load generator on the same machine, installed from the published images.
Short CI run of 30 October 2026: 15 s at 100/s after a 3 s warm-up, then 10 s at maximum
throughput with 32 concurrent callers. The inputs are each pack's own test scenarios.

| Pack / decision | p50 at 100/s | p95 | p99 | Maximum throughput | p95 at maximum |
| --- | --- | --- | --- | --- | --- |
| kyc-risk-rating / risk-rating | 2.2 ms | 2.8 ms | 3.1 ms | 3,747/s | 14.5 ms |
| mobile-money-limits / transaction-limits | 1.6 ms | 11.1 ms | 19.1 ms | 6,145/s | 9.2 ms |
| retail-credit / scorecard | 2.5 ms | 14.0 ms | 22.5 ms | 2,861/s | 18.9 ms |
| sme-treasury / evaluation | 2.3 ms | 12.7 ms | 21.1 ms | 2,932/s | 18.8 ms |
| retail-credit / credit-application, calling a mocked bureau | 2.6 ms | 4.5 ms | 16.1 ms | 2,042/s | 30.0 ms |

No request failed. Latency at 100/s moves between runs on a shared runner (p99 from 3 to 20 ms
for the same decision), always well within the target, so CI holds it to the targets and
compares only the maximum throughput with the baseline.

**Feed.** 1,500 decisions at 100/s: the last was in Studio's log 1.1 s after the load ended.

**Studio stopped.** 30 s at 100/s, Studio stopped for 15 s in the middle:

| Runtime queue | Decisions failed | p99 | Records lost |
| --- | --- | --- | --- |
| 10,000 (default) | 0 | 2.8 ms | 0 |
| 500 | 0 | 2.8 ms | 2,200 of 3,000, counted in the Runtime's log |

The Runtime never waits for Studio: when its queue is full it drops the newest records and logs
the count (`"dropped"`, at each power of two), so decisions keep their latency. Size
`DECISION_LOG__QUEUE_CAPACITY` for the longest Studio outage you plan for: at 100/s, the default
10,000 holds 100 seconds.

## Measure it yourself

On a test server, from a clone of this repository: `scripts/check-install.sh` installs Donka
from the images with the demo (its fake bureau included), then `scripts/check-performance.sh`
loads it. Use a server of its own, not your production installation.

```bash
DONKA_INSTALL_DIR=/opt/donka scripts/check-install.sh <studio tag> <runtime tag> demo
scripts/check-performance.sh /opt/donka full     # 60 s at 100/s, 30 s at maximum
```

The results go to `performance.json` in the repository folder and are checked against the
targets. To load one decision of your own, `donka-admin load-test` in the Studio image does the
same measure:

```bash
docker compose run --rm -e DONKA_RUNTIME_TOKEN=<token> -v "$PWD/inputs.json:/inputs.json:ro" app \
  donka-admin load-test --url http://runtime:8080/api/projects/<project>/evaluate/<decision> \
  --inputs /inputs.json --rate 100 --seconds 60
```

`--rate` sends on a fixed schedule and reports the latency at that load; `--concurrency <n>`
sends as fast as the Runtime answers and reports the maximum throughput. `inputs.json` is a
JSON array of inputs, sent in turn.

## Studio search and replay

The Studio measurement runs **before** the Runtime load and outage checks, on the same
isolated installation made from the images. It sends synthetic records through the real
`/api/v1/decision-log/records` feed in batches of at most 1,000: Studio validates them, encrypts
the payloads and creates their monthly partitions. No raw SQL insertion bypasses that path.
Each installation check creates a separate synthetic project, so asynchronous Runtime calibration
records cannot change its corpus. The output is taken from an actual evaluation of the KYC starter pack's entry decision.
One record in ten is a synthetic failure so the status filter is exercised too.

The short CI run seeds **100,000 records across 60 calendar months**; `full` seeds **1,000,000**.
The rows are spread evenly, with the remainder in the newest months. Historical rows use
mid-month timestamps so month ends and leap days cannot push them outside 1,825-day retention. These are **samples**, not
a five-year, 180-million-record capacity certification: they do not reproduce 100,000 records
per day or varied customer payload sizes. A full-retention measurement needs a separately
provisioned machine and data set before claiming that capacity. `sample.studio` in
`performance.json` defines these sizes, the span, the request counts and the warm-up.

Each case now makes **three rounds of 1,000 measured HTTP requests**, with 50 warm-ups before
each round, in both modes. All 3,000 measurements are pooled for p50/p95/p99; no round is
discarded and no fastest round is selected. The cases are:
the newest page, lookup by reference, decision plus recent date range, failed status, and
replay of successful records spread across old and recent partitions. Search timings include
both the page and its **exact total count**, transaction, JSON response and network transfer;
they are not a timing of the index lookup alone. Replay includes opening and decrypting the
record, loading its recorded release, evaluation and audit. A successful HTTP status is not
enough: filters must return matching records and replay must return the identical output.

`studio.search` (one report per filter) and `studio.replay` contain p50/p95/p99, the maximum,
request count and errors in the same result artifact and a Studio performance notice in the CI job. `studio.corpus` records the actual totals
before and after seeding and every month's row count; `studio.machine` includes the CPU model.
The artifact retains every partition; the notice keeps their count and all timing summaries
to stay below GitHub's 4,096-character limit. A separate Runtime performance notice preserves
its latency, throughput, feed and outage measures, and a failed gate names its cause in a check annotation.
Each case also retains its individual round summaries in `rounds`. Any error, search p95 above
1,000 ms or replay p95 above 2,000 ms, pooled or in any round, fails the check.

`baseline.studio` records the sample settings and actual corpus size, with a `profiles` entry
for each measured CPU model. A comparison requires the same CPU model, virtual core count,
architecture and Node major version, plus identical sample settings and corpus size. CPU names
ignore case and extra spaces; Node updates within the same major remain comparable.
The **20% p95 margin is unchanged** and applies to every search case and replay.
The hardware policy is recorded in [ADR-008](adr/008-hardware-matched-performance-baselines.md).

Every CI run requires a comparable profile: an unknown CPU, different corpus/request count,
missing metric or ambiguous profile fails the check after the result has been saved. A local
run without a comparable profile still checks every absolute target and explicitly reports
that it did not compare a baseline. This also permits a local `full` measurement without
pretending the 100,000-record baseline covers its larger corpus.

### CPU-specific reference results, 30–31 October 2026

Four independent installations from the images, on GitHub `ubuntu-latest` (4 vCPU, 16 GB,
x64, Node 22.23.4), each held exactly **100,000 records across 60 monthly partitions** in a
separate, initially empty project. Each case measured 3,000 requests in three rounds of 1,000,
after 50 warm-ups per round. All **60,000 measured requests succeeded** and every replay
returned the recorded output. The Runtime, feed and outage checks also passed in the first
three runs. The fourth run saved its complete Studio result and stopped at the missing-profile gate, so its later Runtime
checks did not run in that attempt.

| Case, pooled p95 | AMD EPYC 7763 | AMD EPYC 9V74 | Intel Xeon Platinum 8573C | AMD EPYC 9V45 |
| --- | --- | --- | --- | --- |
| Newest page | 21.78 ms | 19.07 ms | 15.08 ms | 19.27 ms |
| Reference lookup | 5.57 ms | 4.28 ms | 3.96 ms | 5.63 ms |
| Decision and recent date | 5.00 ms | 3.10 ms | 3.40 ms | 4.33 ms |
| Failed status | 39.64 ms | 35.54 ms | 23.30 ms | 40.87 ms |
| Replay | 6.06 ms | 4.36 ms | 4.24 ms | 5.37 ms |

Initial sources on commit `dbcf8f1`:

- [Attempt 1, EPYC 7763](https://github.com/youmssi/donka/actions/runs/41121411427/attempts/1).
- [Attempt 2, EPYC 9V74](https://github.com/youmssi/donka/actions/runs/41121411427/attempts/2).
- [Attempt 3, Xeon 8573C](https://github.com/youmssi/donka/actions/runs/41121411427/attempts/3).
- [EPYC 9V45 calibration, commit `caaa056`](https://github.com/youmssi/donka/actions/runs/41183908484/attempts/1): the expected missing-profile failure; absolute Studio checks passed before it.

Each column supplies its own profile in `performance.json`, using that CPU's pooled p95 from
all three rounds. Values from different CPU models are not averaged together. Seeding is
excluded from request timings; complete p50/p95/p99, maxima, round timings, corpus counts and
machine metadata remain in each run's artifact and compact check notice. Subsequent CI runs
validate against the committed profile matching the hardware GitHub assigns.

The initial 50-request trial's replay p95 moved from
[8.47 ms](https://github.com/youmssi/donka/actions/runs/41084840154) to
[13.40 ms](https://github.com/youmssi/donka/actions/runs/41088061492/attempts/1), failing the
20% gate despite passing the absolute target. This led to larger, repeated samples.
Even with those samples, averaging the initial three CPU models would make four unchanged-code
cases exceed their shared mean by 21–31%. CPU-specific comparison retains the 20% rule without
using another hardware model as the reference. Matching a model does not fix all scheduling
or host-load variation; the measured profile and the absolute targets both remain enforced.

To calibrate a new model, review its complete failed-CI report: verify an empty starting
project, the expected corpus/settings, all three rounds, zero errors and every absolute
target. Add the measured hardware profile and its source run to `performance.json` through
the normal PR workflow, then validate with CI. Do not substitute a different model's numbers
or increase an existing profile to conceal a regression. Results are saved before a missing
profile fails, so calibration does not require disabling a gate.

The DNK-66 publication criterion remains open until the product documentation site publishes
these results. This sample does not establish full-retention capacity.

The tool requires Node.js 22 or later, with no installed packages. `check-performance.sh` sets
its `DONKA_PERFORMANCE_*` variables (listed in `.env.example`) and runs
`scripts/studio-performance.mjs --disposable`. That flag acknowledges it **adds synthetic
records**; never use a customer database. Each run issues then revokes its own feed token and
signs out its session. It leaves the synthetic rows in the disposable installation so they can
be inspected; another run reports the enlarged corpus rather than pretending the rows vanished.
