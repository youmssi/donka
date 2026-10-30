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
1,000 ms or replay p95 above 2,000 ms, pooled or in any round, fails the check. Once a Studio
baseline is measured on the reference runner, `baseline.studio` uses the same report shape;
the 20% p95 regression gate compares identical sample settings (including rounds) and actual corpus sizes.
A Runtime baseline never substitutes for a Studio baseline.

### Initial 50-request results, 30 October 2026

[CI run 41084840154](https://github.com/youmssi/donka/actions/runs/41084840154), commit `83b01d5`: images built by the CI image job,
installed with the demo on the reference 4-vCPU, 16-GB GitHub runner (AMD EPYC 7763,
Node 22.23.4). A separate project held exactly **100,000 records in 60 monthly
partitions**, starting from an empty log. Seeding took 128.7 seconds and is
excluded from the request timings. Each case has 50 measured requests after five warm-ups.

| Measure | p50 | p95 | p99 |
| --- | --- | --- | --- |
| Newest page | 23.18 ms | 33.03 ms | 34.48 ms |
| Reference lookup | 5.74 ms | 18.48 ms | 18.78 ms |
| Decision and recent date | 4.33 ms | 17.99 ms | 18.64 ms |
| Failed status | 39.75 ms | 51.38 ms | 52.34 ms |
| Replay | 5.61 ms | 8.47 ms | 12.35 ms |

No measured request failed; every replay returned the recorded output. The existing Runtime,
feed and outage checks also passed in that image job. These values are the original
50-request baseline. [Validation run 41088061492](https://github.com/youmssi/donka/actions/runs/41088061492/attempts/1)
met the absolute targets with zero errors, but replay p95 was **13.40 ms**, 58% above 8.47 ms,
and correctly failed the 20% gate. Its search p95 values were 33.35, 21.22, 17.96 and 51.61 ms,
respectively. The CPU model, sample and corpus matched; application source had not changed.
These observations alone do not identify the cause of the difference. One controlled
[same-head repeat](https://github.com/youmssi/donka/actions/runs/41088061492/attempts/2)
passed with replay p95 8.42 ms on a newer AMD EPYC 9V45 CPU. It does not explain the original
same-model difference or replace the failed measurement.

The owner confirmed keeping the 20% gate and calibrating with larger, repeated samples.
The revised method above is awaiting three independent CI measurements. Their pooled p95s
will be averaged, using all three runs, to form the new baseline; a separate run will validate
it. The old 50-request baseline remains recorded until then and is not compared against the
different request count. No baseline has been raised to make the failed run pass.
The separate DNK-66 publication criterion remains open until the product documentation site
publishes these results. This sample does not establish full-retention capacity.

The tool requires Node.js 22 or later, with no installed packages. `check-performance.sh` sets
its `DONKA_PERFORMANCE_*` variables (listed in `.env.example`) and runs
`scripts/studio-performance.mjs --disposable`. That flag acknowledges it **adds synthetic
records**; never use a customer database. Each run issues then revokes its own feed token and
signs out its session. It leaves the synthetic rows in the disposable installation so they can
be inspected; another run reports the enlarged corpus rather than pretending the rows vanished.
