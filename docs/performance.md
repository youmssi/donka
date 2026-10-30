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
| Studio decision-log search and replay | p95 1 s and 2 s (measured in a later change) |
| Regression | a maximum throughput more than 20% below its baseline fails CI |

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
