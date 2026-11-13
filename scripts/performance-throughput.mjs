#!/usr/bin/env node
/** Summarize independent maximum-throughput windows for the DNK-67 CI gate. */
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const SAMPLE_COUNT = 3;

export function summarizeMaximumSamples(samples) {
  if (!Array.isArray(samples) || samples.length !== SAMPLE_COUNT) {
    throw new TypeError(`Expected exactly ${SAMPLE_COUNT} maximum-throughput samples`);
  }

  for (const [index, sample] of samples.entries()) {
    if (!sample || typeof sample !== 'object' || Array.isArray(sample)) {
      throw new TypeError(`Sample ${index + 1} must be an object`);
    }
    for (const field of ['sent', 'answered', 'requests', 'errors', 'throughput']) {
      if (!Number.isFinite(sample[field]) || sample[field] < 0) {
        throw new TypeError(`Sample ${index + 1} has an invalid ${field}`);
      }
    }
    if (
      !Number.isInteger(sample.sent) ||
      !Number.isInteger(sample.answered) ||
      !Number.isInteger(sample.requests) ||
      !Number.isInteger(sample.errors) ||
      sample.requests === 0 ||
      sample.throughput === 0
    ) {
      throw new TypeError(`Sample ${index + 1} is incomplete`);
    }
  }

  const ordered = [...samples].sort((left, right) => left.throughput - right.throughput);
  return {
    sent: samples.reduce((total, sample) => total + sample.sent, 0),
    answered: samples.reduce((total, sample) => total + sample.answered, 0),
    requests: samples.reduce((total, sample) => total + sample.requests, 0),
    errors: samples.reduce((total, sample) => total + sample.errors, 0),
    throughput: ordered[1].throughput,
    sampleCount: SAMPLE_COUNT,
    samples,
  };
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  let input = '';
  for await (const chunk of process.stdin) input += chunk;
  const lines = input.trim().split(/\r?\n/);
  const samples = lines.map((line) => JSON.parse(line));
  process.stdout.write(`${JSON.stringify(summarizeMaximumSamples(samples))}\n`);
}
