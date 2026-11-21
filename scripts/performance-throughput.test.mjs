import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { summarizeMaximumSamples } from './performance-throughput.mjs';

const report = (throughput, overrides = {}) => ({
  sent: 13_000,
  answered: 13_000,
  requests: 10_000,
  errors: 0,
  throughput,
  p50_ms: 1,
  p95_ms: 2,
  p99_ms: 3,
  max_ms: 4,
  ...overrides,
});

test('uses the median maximum throughput and retains every window', () => {
  const samples = [report(4_265), report(6_100), report(5_900)];
  const result = summarizeMaximumSamples(samples);

  assert.equal(result.throughput, 5_900);
  assert.equal(result.sampleCount, 3);
  assert.deepEqual(result.samples, samples);
});

test('sums request accounting without treating samples as one longer throughput window', () => {
  const result = summarizeMaximumSamples([
    report(4_000),
    report(5_000, {
      sent: 13_002,
      answered: 13_000,
      requests: 10_000,
      errors: 1,
    }),
    report(6_000),
  ]);

  assert.equal(result.sent, 39_002);
  assert.equal(result.answered, 39_000);
  assert.equal(result.requests, 30_000);
  assert.equal(result.errors, 1);
  assert.equal(result.throughput, 5_000);
});

test('requires all three independent windows', () => {
  assert.throws(() => summarizeMaximumSamples([report(4_000), report(5_000)]), /exactly 3/);
  assert.throws(() => summarizeMaximumSamples([report(4_000), report(5_000), null]), /Sample 3/);
});

test('rejects a sample with missing or invalid throughput metrics', () => {
  assert.throws(
    () => summarizeMaximumSamples([report(4_000), report(Number.NaN), report(6_000)]),
    /invalid throughput/,
  );
  assert.throws(() => summarizeMaximumSamples([report(4_000), report(0), report(6_000)]), /incomplete/);
});

test('CLI aggregates newline-delimited load reports', () => {
  const samples = [report(4_000), report(5_000), report(6_000)];
  const command = spawnSync(
    process.execPath,
    [fileURLToPath(new URL('./performance-throughput.mjs', import.meta.url))],
    {
      encoding: 'utf8',
      input: `${samples.map((sample) => JSON.stringify(sample)).join('\n')}\n`,
    },
  );

  assert.equal(command.status, 0, command.stderr);
  assert.equal(JSON.parse(command.stdout).throughput, 5_000);
});
