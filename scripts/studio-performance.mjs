#!/usr/bin/env node
/** Studio search and replay on synthetic records sent through the encrypted feed (DNK-67). */
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import { cpus } from 'node:os';
import { performance } from 'node:perf_hooks';
import { pathToFileURL } from 'node:url';

/** A unique synthetic project key within Studio's 40-character contract. */
export function projectKey() {
  return `perf-${randomUUID().replaceAll('-', '')}`;
}

export function distribution(records, months, now) {
  assert.ok(Number.isSafeInteger(records) && records >= months, 'Use at least one record per month');
  assert.ok(Number.isSafeInteger(months) && months >= 2 && months <= 60, 'Use 2 to 60 months');
  return Array.from({ length: months }, (_, offset) => {
    // A month-start five calendar years back can fall outside the 1825-day retention
    // window at month end or after a leap day. Mid-month keeps historical rows inside it.
    const start = Date.UTC(now.getUTCFullYear(), now.getUTCMonth() - offset, offset === 0 ? 1 : 15);
    return {
      at: new Date(start).toISOString(),
      records: Math.floor(records / months) + Number(offset < records % months),
    };
  });
}

export function summary(latencies, errors) {
  assert.ok(latencies.length > 0, 'No requests measured');
  const sorted = latencies.toSorted((a, b) => a - b);
  const at = (p) => Math.round(sorted[Math.ceil(sorted.length * p) - 1] * 100) / 100;
  return { requests: sorted.length, errors, p50_ms: at(0.5), p95_ms: at(0.95), p99_ms: at(0.99), max_ms: at(1) };
}

/** Keep the CI notice below GitHub's 4096-character limit; the artifact retains every partition. */
export function noticeReport(report) {
  const { partitions, ...corpus } = report.corpus;
  return { ...report, corpus: { ...corpus, partitionCount: partitions.length } };
}

export function checkTargets(report, config) {
  const targets = config.targets.studio;
  const baseline = config.baseline?.studio;
  for (const [name, measured] of Object.entries({ ...report.search, replay: report.replay })) {
    const target = name === 'replay' ? targets.replay_p95_ms : targets.search_p95_ms;
    for (const [index, result] of [measured, ...(measured.rounds ?? [])].entries()) {
      const label = index === 0 ? name : `${name} round ${index}`;
      assert.equal(result.errors, 0, `${label}: ${result.errors} failed requests`);
      assert.ok(result.p95_ms <= target, `${label}: p95 ${result.p95_ms} ms exceeds target ${target} ms`);
    }
    // A baseline is comparable only at the same sample size, partition span and request count.
    if (
      baseline &&
      ['records', 'months', 'samples', 'warmup', 'rounds'].every(
        (key) => baseline.sample?.[key] === report.sample[key],
      ) &&
      baseline.corpus?.after === report.corpus.after
    ) {
      const previous = name === 'replay' ? baseline.replay : baseline.search?.[name];
      if (previous) {
        assert.ok(measured.p95_ms <= previous.p95_ms * (1 + config.margin), `${name}: p95 exceeds the baseline margin`);
      }
    }
  }
}

export async function run(options) {
  const {
    studioUrl,
    email,
    password,
    project,
    release,
    decision,
    input,
    output,
    records,
    months,
    samples,
    warmup,
    rounds,
  } = options;
  assert.ok(Number.isSafeInteger(samples) && samples > 0, 'samples must be positive');
  assert.ok(Number.isSafeInteger(warmup) && warmup >= 0, 'warmup must be nonnegative');
  assert.ok(Number.isSafeInteger(rounds) && rounds > 0, 'rounds must be positive');
  const span = distribution(records, months, options.now ?? new Date());
  const base = new URL('/api/v1/', studioUrl);
  assert.ok(['http:', 'https:'].includes(base.protocol) && !base.username && !base.password, 'Invalid Studio origin');
  let cookie = '',
    token;
  const api = async (method, path, body, bearer) => {
    const response = await fetch(new URL(path, base), {
      method,
      headers: {
        'content-type': 'application/json',
        ...(bearer ? { authorization: `Bearer ${bearer}` } : { cookie, 'x-donka-csrf': '1' }),
      },
      ...(body !== undefined && { body: JSON.stringify(body) }),
      signal: AbortSignal.timeout(30_000),
      redirect: 'error',
    });
    if (path === 'auth/sign-in') {
      cookie = response.headers
        .getSetCookie()
        .map((value) => value.split(';')[0])
        .join('; ');
    }
    assert.ok(response.ok, `${method} ${path.split('?')[0]}: HTTP ${response.status}`);
    return response.status === 204 ? null : response.json();
  };
  const measure = async (call) => {
    const latencies = [];
    const measuredRounds = [];
    let errors = 0;
    for (let round = 0; round < rounds; round++) {
      const current = [];
      let roundErrors = 0;
      for (let i = 0; i < warmup + samples; i++) {
        const start = performance.now();
        try {
          await call(i);
        } catch (error) {
          if (i < warmup) throw error;
          roundErrors++;
        }
        if (i >= warmup) current.push(performance.now() - start);
      }
      measuredRounds.push(summary(current, roundErrors));
      latencies.push(...current);
      errors += roundErrors;
    }
    // Pool every request, including failed measurements; never select the fastest round.
    return { ...summary(latencies, errors), rounds: measuredRounds };
  };
  const started = performance.now();
  const path = `projects/${encodeURIComponent(project)}/decision-log`;
  const prefix = `performance-${randomUUID()}`;
  const replayIds = [];
  let seeded = 0;
  try {
    await api('POST', 'auth/sign-in', { email, password });
    assert.ok(cookie, 'Studio did not issue a session');
    token = await api('POST', 'decision-log/tokens', { environment: 'staging', name: prefix });
    const before = await api('GET', `${path}?limit=1`);
    for (const [month, part] of span.entries()) {
      for (let offset = 0; offset < part.records; offset += 1000) {
        const batch = Array.from({ length: Math.min(1000, part.records - offset) }, (_, index) => {
          const number = seeded + index;
          const failed = (offset + index) % 10 === 9;
          const id = randomUUID();
          // Replay one evenly spaced record per sample, spanning old and recent partitions.
          if (!failed && (offset + index) % Math.max(1, Math.floor(part.records / samples)) === 0) replayIds.push(id);
          return {
            id,
            projectId: project,
            releaseId: release,
            environment: 'staging',
            decisionKey: decision,
            reference: `${prefix}-${number}`,
            evaluatedAt: part.at,
            durationUs: 1000,
            status: failed ? 'failed' : 'succeeded',
            input,
            ...(failed ? { error: { code: 'SYNTHETIC_FAILURE' } } : { output }),
          };
        });
        const receipt = await api('POST', 'decision-log/records', { records: batch }, token.token);
        assert.equal(receipt.accepted, batch.length, 'Feed did not accept the whole batch');
        assert.deepEqual(receipt.rejected, [], 'Feed rejected synthetic records');
        seeded += batch.length;
      }
      options.progress?.(`seeded ${seeded}/${records} records (${month + 1}/${months} months)`);
    }
    const after = await api('GET', `${path}?limit=1`);
    assert.ok(after.total >= before.total + records, 'Search does not see the seeded corpus');
    assert.ok(replayIds.length > 0, 'No successful records available for replay');
    const seedSeconds = (performance.now() - started) / 1000;
    const search = {};
    const cases = {
      newest: { query: '', validate: (page) => assert.ok(page.total >= records && page.items.length > 0) },
      reference: {
        query: `reference=${prefix}-${Math.floor(records / 2)}`,
        validate: (page) => {
          assert.equal(page.total, 1);
          assert.equal(page.items.length, 1);
          assert.equal(page.items[0].reference, `${prefix}-${Math.floor(records / 2)}`);
        },
      },
      decisionAndDate: {
        query: new URLSearchParams({ decisionKey: decision, from: span[0].at }).toString(),
        validate: (page) => {
          assert.ok(page.total >= span[0].records && page.items.length > 0);
          for (const item of page.items) assert.ok(item.decisionKey === decision && item.evaluatedAt >= span[0].at);
        },
      },
      status: {
        query: 'status=failed',
        validate: (page) => {
          assert.ok(page.total > 0 && page.items.length > 0);
          for (const item of page.items) assert.equal(item.status, 'failed');
        },
      },
    };
    for (const [name, test] of Object.entries(cases)) {
      search[name] = await measure(async () => test.validate(await api('GET', `${path}?limit=20&${test.query}`)));
    }
    const replay = await measure(async (index) => {
      const id = replayIds[Math.floor((index * replayIds.length) / (warmup + samples))];
      const result = await api('POST', `${path}/${id}/replay`);
      assert.equal(result.identical, true, 'Replay differs from the stored decision');
      assert.equal(result.status, 'succeeded');
      assert.deepEqual(result.output, output);
    });
    return {
      measuredAt: new Date().toISOString(),
      sample: { records, months, samples, warmup, rounds },
      corpus: { before: before.total, after: after.total, partitions: span, seedSeconds },
      search,
      replay,
      machine: { cpus: cpus().length, cpuModel: cpus()[0]?.model, arch: process.arch, node: process.versions.node },
    };
  } finally {
    try {
      if (token) await api('DELETE', `decision-log/tokens/${token.id}`);
    } finally {
      if (cookie) await api('POST', 'auth/sign-out');
    }
  }
}

async function main() {
  assert.equal(
    process.argv[2],
    '--disposable',
    'Pass --disposable only for a test installation; this adds synthetic records',
  );
  assert.ok(
    process.argv.length <= 4 && [undefined, 'full'].includes(process.argv[3]),
    'Usage: studio-performance.mjs --disposable [full]',
  );
  const config = JSON.parse(await readFile(new URL('../docs/performance.json', import.meta.url), 'utf8'));
  const full = process.argv[3] === 'full';
  const setting = config.sample.studio;
  const required = (name) => {
    const value = process.env[name];
    assert.ok(value, `Set ${name}`);
    return value;
  };
  const fixture = JSON.parse(await readFile(required('DONKA_PERFORMANCE_FIXTURE'), 'utf8'));
  const out = required('DONKA_PERFORMANCE_STUDIO_OUT');
  const report = await run({
    studioUrl: required('DONKA_PERFORMANCE_STUDIO_URL'),
    email: required('DONKA_PERFORMANCE_EMAIL'),
    password: required('DONKA_PERFORMANCE_PASSWORD'),
    project: required('DONKA_PERFORMANCE_PROJECT'),
    release: required('DONKA_PERFORMANCE_RELEASE'),
    decision: required('DONKA_PERFORMANCE_DECISION'),
    input: fixture.input,
    output: fixture.output,
    records: full ? setting.full_records : setting.records,
    months: setting.months,
    samples: full ? setting.full_samples : setting.samples,
    warmup: setting.warmup,
    rounds: setting.rounds,
    progress: (message) => console.error(`studio-performance: ${message}`),
  });
  await writeFile(out, JSON.stringify({ studio: report }, null, 2) + '\n');
  console.log(`studio-performance: ${JSON.stringify(report)}`);
  // Keep the measured corpus and percentiles visible in the job's checks as well as its artifact.
  if (process.env.GITHUB_ACTIONS === 'true') {
    const notice = JSON.stringify(noticeReport(report))
      .replaceAll('%', '%25')
      .replaceAll('\r', '%0D')
      .replaceAll('\n', '%0A');
    console.log(`::notice title=Studio performance::${notice}`);
  }
  checkTargets(report, config);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) await main();
