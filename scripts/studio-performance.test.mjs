import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { once } from 'node:events';
import { createServer } from 'node:http';
import test from 'node:test';
import { checkTargets, distribution, run, summary } from './studio-performance.mjs';

const targets = { targets: { studio: { search_p95_ms: 1000, replay_p95_ms: 2000 } }, margin: 0.2 };

test('the corpus spans every retained month without inventing extra records', () => {
  const span = distribution(100003, 60, new Date('2026-10-01T00:00:00Z'));
  assert.equal(span.length, 60);
  assert.equal(
    span.reduce((count, part) => count + part.records, 0),
    100003,
  );
  assert.equal(span[0].at, '2026-10-01T00:00:00.000Z');
  assert.equal(span[59].at, '2021-11-01T00:00:00.000Z');
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

test('targets and comparable baselines fail on errors, latency and regression', () => {
  const report = {
    sample: { records: 600 },
    corpus: { after: 600 },
    search: { newest: summary([500], 0) },
    replay: summary([1000], 0),
  };
  checkTargets(report, targets);
  assert.throws(() => checkTargets({ ...report, replay: summary([2001], 0) }, targets), /exceeds target/);
  assert.throws(() => checkTargets({ ...report, replay: summary([1], 1) }, targets), /failed requests/);
  const config = {
    ...targets,
    baseline: { studio: { sample: report.sample, corpus: report.corpus, search: { newest: { p95_ms: 400 } } } },
  };
  assert.throws(() => checkTargets(report, config), /baseline margin/);
  checkTargets({ ...report, sample: { records: 1000 } }, config);
});

async function studio(t, { rejectFeed = false, wrongReplay = false } = {}) {
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
        return send({ identical: !wrongReplay, status: 'succeeded', output: record.output });
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
      now: new Date('2026-10-30T12:00:00Z'),
    },
  };
}

test('seed, filtered searches and replay exercise all monthly partitions and revoke credentials', async (t) => {
  const app = await studio(t);
  const report = await run(app.options);
  checkTargets(report, targets);
  assert.equal(app.records.length, 600);
  assert.equal(app.records.filter((record) => record.status === 'failed').length, 60);
  assert.equal(new Set(app.records.map((record) => record.evaluatedAt)).size, 60);
  assert.equal(new Set(app.replayed.map((record) => record.evaluatedAt)).size, 60);
  assert.equal(report.replay.requests, 60);
  for (const measured of Object.values(report.search)) assert.equal(measured.requests, 60);
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
  assert.equal(report.replay.errors, 2);
  assert.throws(() => checkTargets(report, targets), /failed requests/);
});

test('the command refuses seeding without explicit disposable-installation acknowledgement', async () => {
  await assert.rejects(
    promisify(execFile)(process.execPath, [new URL('./studio-performance.mjs', import.meta.url).pathname]),
    (error) => error.code === 1 && error.stderr.includes('Pass --disposable'),
  );
});
