import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { once } from 'node:events';
import { createServer } from 'node:http';
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { checkTargets, distribution, noticeReport, projectKey, run, summary } from './studio-performance.mjs';

const targets = { targets: { studio: { search_p95_ms: 1000, replay_p95_ms: 2000 } }, margin: 0.2 };
const machine = { cpus: 4, cpuModel: 'Synthetic CPU', arch: 'x64', node: '22.23.4' };

test('the corpus spans every retained month without inventing extra records', () => {
  const span = distribution(100003, 60, new Date('2026-10-01T00:00:00Z'));
  assert.equal(span.length, 60);
  assert.equal(
    span.reduce((count, part) => count + part.records, 0),
    100003,
  );
  assert.equal(span[0].at, '2026-10-01T00:00:00.000Z');
  assert.equal(span[59].at, '2021-11-15T00:00:00.000Z');
  assert.equal(new Set(span.map((part) => part.at)).size, 60);
  assert.throws(() => distribution(1, 60, new Date()), /one record per month/);
});

test('percentiles use nearest rank and empty measurements cannot pass', () => {
  const measured = summary(
    Array.from({ length: 100 }, (_, i) => i + 1),
    2,
  );
  assert.deepEqual(measured, { requests: 100, errors: 2, p50_ms: 50, p95_ms: 95, p99_ms: 99, max_ms: 100 });
  assert.throws(() => summary([], 0), /No requests/);
});

test('CI notices retain all measured rounds without exceeding GitHub’s annotation limit', () => {
  const round = summary([1000, 2000, 3000], 0);
  const measured = { ...round, requests: 3000, rounds: [round, round, round] };
  const report = {
    measuredAt: '2026-10-30T20:00:00.000Z',
    sample: { records: 100000, months: 60, samples: 1000, warmup: 50, rounds: 3 },
    corpus: {
      before: 0,
      after: 100000,
      seedSeconds: 128.513285568,
      partitions: distribution(100000, 60, new Date('2026-10-30')),
    },
    search: { newest: measured, reference: measured, decisionAndDate: measured, status: measured },
    replay: measured,
    machine: { cpus: 4, cpuModel: 'AMD EPYC 7763 64-Core Processor', arch: 'x64', node: '22.23.4' },
  };
  const notice = noticeReport(report);
  const serialized = JSON.stringify(notice);
  assert.ok(Buffer.byteLength(serialized) < 4096);
  assert.deepEqual(JSON.parse(serialized).replay, report.replay);
  assert.deepEqual(notice.search, report.search);
  assert.equal(notice.corpus.partitionCount, 60);
  assert.equal(notice.corpus.after, 100000);
  assert.equal(report.corpus.partitions.length, 60, 'the full artifact must retain its partition details');
});

test('targets and comparable baselines fail on errors, latency and regression', () => {
  const report = {
    machine,
    sample: { records: 600, months: 60, samples: 1, warmup: 0, rounds: 1 },
    corpus: { after: 600 },
    search: { newest: summary([500], 0) },
    replay: summary([1000], 0),
  };
  checkTargets(report, targets);
  assert.throws(() => checkTargets({ ...report, replay: summary([2001], 0) }, targets), /exceeds target/);
  assert.throws(() => checkTargets({ ...report, replay: summary([1], 1) }, targets), /failed requests/);
  const config = {
    ...targets,
    baseline: {
      studio: {
        sample: report.sample,
        corpus: report.corpus,
        profiles: [{ machine, search: { newest: { p95_ms: 400 } }, replay: { p95_ms: 1000 } }],
      },
    },
  };
  assert.throws(() => checkTargets(report, config), /baseline margin/);
  checkTargets({ ...report, sample: { records: 1000 } }, config);
  checkTargets({ ...report, sample: { ...report.sample, rounds: 3 } }, config);
  assert.throws(
    () => checkTargets({ ...report, replay: { ...report.replay, rounds: [summary([2001], 0)] } }, targets),
    /replay round 1.*exceeds target/,
  );
});

test('the 20 percent gate selects the matching CPU instead of the first available profile', () => {
  const sample = { records: 100000, months: 60, samples: 1000, warmup: 50, rounds: 3 };
  const corpus = { after: 100000 };
  const profile = (cpuModel, p95) => ({
    machine: { ...machine, cpuModel },
    search: { newest: { p95_ms: p95 } },
    replay: { p95_ms: 1000 },
  });
  const config = {
    ...targets,
    baseline: { studio: { sample, corpus, profiles: [profile('Fast CPU', 400), profile('Slow CPU', 800)] } },
  };
  const report = {
    machine: { ...machine, cpuModel: 'Slow CPU' },
    sample,
    corpus,
    search: { newest: summary([500], 0) },
    replay: summary([1000], 0),
  };
  assert.equal(checkTargets(report, config, { requireBaseline: true }), true);
  assert.throws(
    () => checkTargets({ ...report, machine: { ...machine, cpuModel: 'Fast CPU' } }, config),
    /baseline margin/,
  );
  const fast = { ...report, machine: { ...machine, cpuModel: 'Fast CPU' }, search: { newest: summary([480], 0) } };
  assert.equal(checkTargets(fast, config), true);
  assert.throws(() => checkTargets({ ...fast, search: { newest: summary([480.01], 0) } }, config), /baseline margin/);
  assert.equal(checkTargets({ ...fast, replay: summary([1200], 0) }, config), true);
  assert.throws(() => checkTargets({ ...fast, replay: summary([1200.01], 0) }, config), /baseline margin/);
});

test('CI rejects unknown hardware, client major versions, sample sizes and incomplete baselines', () => {
  const sample = { records: 100000, months: 60, samples: 1000, warmup: 50, rounds: 3 };
  const corpus = { after: 100000 };
  const profile = { machine, search: { newest: { p95_ms: 500 } }, replay: { p95_ms: 1000 } };
  const config = { ...targets, baseline: { studio: { sample, corpus, profiles: [profile] } } };
  const report = { machine, sample, corpus, search: { newest: summary([500], 0) }, replay: summary([1000], 0) };
  for (const change of [
    { machine: { ...machine, cpuModel: 'Unknown CPU' } },
    { machine: { ...machine, cpus: 8 } },
    { machine: { ...machine, arch: 'arm64' } },
    { machine: { ...machine, node: '24.1.0' } },
    { sample: { ...sample, samples: 50 } },
    { corpus: { after: 200000 } },
  ]) {
    const different = { ...report, ...change };
    assert.throws(() => checkTargets(different, config, { requireBaseline: true }), /No comparable Studio baseline/);
    assert.equal(checkTargets(different, config), false, 'local measurements still enforce absolute targets');
    assert.throws(() => checkTargets({ ...different, replay: summary([2001], 0) }, config), /exceeds target/);
  }
  assert.equal(
    checkTargets({ ...report, machine: { ...machine, cpuModel: '  SYNTHETIC   CPU ', node: '22.99.0' } }, config),
    true,
  );
  assert.throws(
    () =>
      checkTargets(report, {
        ...config,
        baseline: { studio: { sample, corpus, profiles: [{ ...profile, replay: undefined }] } },
      }),
    /comparable baseline is missing/,
  );
  assert.throws(
    () => checkTargets(report, { ...config, baseline: { studio: { sample, corpus, profiles: [profile, profile] } } }),
    /ambiguous profile/,
  );
});

async function studio(t, { rejectFeed = false, wrongReplay = false, wrongReplayAt } = {}) {
  const records = [];
  const replayed = [];
  const cleanup = [];
  const server = createServer(async (request, response) => {
    try {
      const url = new URL(request.url, 'http://localhost');
      const chunks = [];
      for await (const chunk of request) chunks.push(chunk);
      const body = chunks.length ? JSON.parse(Buffer.concat(chunks)) : null;
      response.setHeader('content-type', 'application/json');
      const send = (value) => response.end(JSON.stringify(value));
      if (url.pathname.endsWith('/auth/sign-in')) {
        assert.deepEqual(body, { email: 'test@example.invalid', password: 'synthetic passphrase' });
        response.setHeader('set-cookie', 'session=synthetic; HttpOnly');
        return send({});
      }
      if (url.pathname.endsWith('/decision-log/records')) {
        assert.equal(request.headers.authorization, 'Bearer synthetic-feed-token');
        if (!rejectFeed) records.push(...body.records);
        return send({
          accepted: rejectFeed ? 0 : body.records.length,
          rejected: rejectFeed ? [{ code: 'invalid' }] : [],
        });
      }
      assert.equal(request.headers.cookie, 'session=synthetic');
      assert.equal(request.headers['x-donka-csrf'], '1');
      if (url.pathname.endsWith('/decision-log/tokens') && request.method === 'POST')
        return send({ id: 'feed-id', token: 'synthetic-feed-token' });
      if (request.method === 'DELETE' || url.pathname.endsWith('/auth/sign-out')) {
        cleanup.push(url.pathname);
        response.statusCode = 204;
        return response.end();
      }
      if (url.pathname.endsWith('/replay')) {
        const id = url.pathname.split('/').at(-2);
        const record = records.find((row) => row.id === id);
        assert.ok(record && record.status === 'succeeded');
        replayed.push(record);
        return send({
          identical: !wrongReplay && replayed.length !== wrongReplayAt,
          status: 'succeeded',
          output: record.output,
        });
      }
      let filtered = records;
      for (const [name, value] of url.searchParams) {
        if (name === 'from') filtered = filtered.filter((record) => record.evaluatedAt >= value);
        if (['reference', 'status', 'decisionKey'].includes(name))
          filtered = filtered.filter((record) => record[name] === value);
      }
      return send({ items: filtered.slice(0, Number(url.searchParams.get('limit'))), total: filtered.length });
    } catch (error) {
      response.statusCode = 500;
      response.end(JSON.stringify({ error: error.message }));
    }
  });
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  t.after(() => new Promise((resolve) => server.close(resolve)));
  return {
    records,
    replayed,
    cleanup,
    options: {
      studioUrl: `http://127.0.0.1:${server.address().port}`,
      email: 'test@example.invalid',
      password: 'synthetic passphrase',
      project: 'project',
      release: 'release',
      decision: 'decision',
      input: { amount: 12 },
      output: { accepted: true },
      records: 600,
      months: 60,
      samples: 60,
      warmup: 1,
      rounds: 3,
      now: new Date('2026-10-30T12:00:00Z'),
    },
  };
}

test('the real command rejects missing CI calibration, preserves its result and reports local absolute-only checks', async (t) => {
  const app = await studio(t);
  const dir = await mkdtemp(join(tmpdir(), 'donka-performance-'));
  t.after(() => rm(dir, { recursive: true, force: true }));
  await mkdir(join(dir, 'scripts'));
  await mkdir(join(dir, 'docs'));
  const script = join(dir, 'scripts/studio-performance.mjs');
  await copyFile(new URL('./studio-performance.mjs', import.meta.url), script);
  await writeFile(
    join(dir, 'docs/performance.json'),
    JSON.stringify({
      ...targets,
      sample: {
        studio: {
          records: 600,
          full_records: 600,
          months: 60,
          samples: 2,
          full_samples: 2,
          warmup: 0,
          rounds: 3,
        },
      },
    }),
  );
  const fixture = join(dir, 'fixture.json'),
    out = join(dir, 'performance.json');
  await writeFile(fixture, JSON.stringify({ input: app.options.input, output: app.options.output }));
  const env = {
    ...process.env,
    GITHUB_ACTIONS: 'true',
    DONKA_PERFORMANCE_STUDIO_URL: app.options.studioUrl,
    DONKA_PERFORMANCE_EMAIL: app.options.email,
    DONKA_PERFORMANCE_PASSWORD: app.options.password,
    DONKA_PERFORMANCE_PROJECT: app.options.project,
    DONKA_PERFORMANCE_RELEASE: app.options.release,
    DONKA_PERFORMANCE_DECISION: app.options.decision,
    DONKA_PERFORMANCE_FIXTURE: fixture,
    DONKA_PERFORMANCE_STUDIO_OUT: out,
  };
  await assert.rejects(promisify(execFile)(process.execPath, [script, '--disposable'], { env }), (error) => {
    assert.equal(error.code, 1);
    assert.match(error.stderr, /No comparable Studio baseline/);
    assert.match(error.stdout, /::error title=Studio performance gate::/);
    return true;
  });
  const saved = JSON.parse(await readFile(out, 'utf8')).studio;
  assert.equal(saved.corpus.after, 600);
  assert.equal(saved.replay.requests, 6);
  assert.equal(saved.replay.errors, 0);
  assert.equal(app.cleanup.length, 2);
  const { stdout } = await promisify(execFile)(process.execPath, [script, '--disposable'], {
    env: { ...env, GITHUB_ACTIONS: 'false' },
  });
  assert.match(stdout, /absolute targets passed; no comparable hardware\/sample baseline/);
});

test('seed, filtered searches and replay exercise all monthly partitions and revoke credentials', async (t) => {
  const app = await studio(t);
  const report = await run(app.options);
  checkTargets(report, targets);
  assert.equal(app.records.length, 600);
  assert.equal(app.records.filter((record) => record.status === 'failed').length, 60);
  assert.equal(new Set(app.records.map((record) => record.evaluatedAt)).size, 60);
  assert.equal(new Set(app.replayed.map((record) => record.evaluatedAt)).size, 60);
  assert.equal(report.replay.requests, 180);
  assert.equal(app.replayed.length, 183, 'each round has its own excluded warm-up');
  for (const measured of [...Object.values(report.search), report.replay]) {
    assert.equal(measured.requests, 180);
    assert.equal(measured.rounds.length, 3);
    assert.ok(measured.rounds.every((round) => round.requests === 60 && round.errors === 0));
  }
  assert.ok(app.records.every((record) => record.projectId === 'project' && record.releaseId === 'release'));
  assert.deepEqual(app.cleanup, ['/api/v1/decision-log/tokens/feed-id', '/api/v1/auth/sign-out']);
  assert.ok(!JSON.stringify(report).includes('synthetic-feed-token'));
});

test('a refused feed cannot become a successful benchmark and still revokes its credential', async (t) => {
  const app = await studio(t, { rejectFeed: true });
  await assert.rejects(run(app.options), /Feed did not accept/);
  assert.equal(app.cleanup.length, 2);
});

test('HTTP success with a wrong replay is counted as a failed measurement', async (t) => {
  const app = await studio(t, { wrongReplay: true });
  const report = await run({ ...app.options, samples: 2, warmup: 0 });
  assert.equal(report.replay.errors, 6);
  assert.ok(report.replay.rounds.every((round) => round.errors === 2));
  assert.throws(() => checkTargets(report, targets), /failed requests/);
});

test('a failure in the last round remains in the pooled result and fails the gate', async (t) => {
  const app = await studio(t, { wrongReplayAt: 3 });
  const report = await run({ ...app.options, samples: 1, warmup: 0 });
  assert.equal(report.replay.requests, 3);
  assert.equal(report.replay.errors, 1);
  assert.deepEqual(
    report.replay.rounds.map((round) => round.errors),
    [0, 0, 1],
  );
  assert.throws(() => checkTargets(report, targets), /replay: 1 failed requests/);
});

test('the command refuses seeding without explicit disposable-installation acknowledgement', async () => {
  await assert.rejects(
    promisify(execFile)(process.execPath, [new URL('./studio-performance.mjs', import.meta.url).pathname]),
    (error) => error.code === 1 && error.stderr.includes('Pass --disposable'),
  );
});

test('the disposable project key satisfies Studio’s public key length and character contract', async () => {
  const first = projectKey();
  assert.ok(first.length <= 40);
  assert.match(first, /^[a-z][a-z0-9]*(-[a-z0-9]+)*$/);
  assert.notEqual(first, projectKey());
  const { stdout } = await promisify(execFile)(process.execPath, [
    '--input-type=module',
    '-e',
    'const {projectKey} = await import(process.argv[1]); process.stdout.write(projectKey())',
    new URL('./studio-performance.mjs', import.meta.url).href,
  ]);
  assert.ok(stdout.length <= 40);
  assert.match(stdout, /^[a-z][a-z0-9]*(-[a-z0-9]+)*$/);
});

test('all sixty months remain inside five-year retention at month end and after leap days', () => {
  for (const timestamp of ['2026-10-31T16:00:00Z', '2028-02-29T16:00:00Z', '2028-03-01T00:00:00Z']) {
    const now = new Date(timestamp);
    const span = distribution(100000, 60, now);
    assert.equal(new Set(span.map((part) => part.at.slice(0, 7))).size, 60);
    for (const part of span) {
      assert.ok(Date.parse(part.at) <= now.getTime());
      assert.ok(Date.parse(part.at) > now.getTime() - 1825 * 86400000, part.at);
    }
  }
});
