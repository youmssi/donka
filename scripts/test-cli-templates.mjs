#!/usr/bin/env node
/** Real Studio + packaged CLI checks for the six CI template script bodies. Linux with bash/sh/jq. */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

function required(name) {
  const value = process.env[name];
  assert.ok(value?.trim(), `Set ${name} for the disposable test installation`);
  return value;
}
assert.notEqual(process.platform, 'win32', 'Run the template scripts in Linux (WSL is supported)');
const origin = new URL(required('DONKA_E2E_BASE_URL'));
assert.ok(
  ['http:', 'https:'].includes(origin.protocol) &&
    !origin.username &&
    !origin.password &&
    origin.pathname === '/' &&
    !origin.search &&
    !origin.hash,
  'Use a Studio origin without credentials or a path',
);
const cli = resolve(required('DONKA_E2E_CLI_DIR'));
const { parse } = createRequire(join(cli, 'package.json'))('yaml');
const work = await mkdtemp(join(tmpdir(), 'donka-cli-templates-'));
const environment = Object.fromEntries(
  ['PATH', 'HOME', 'TMP', 'TEMP'].flatMap((name) => (process.env[name] ? [[name, process.env[name]]] : [])),
);
Object.assign(environment, {
  NO_COLOR: '1',
  npm_config_cache: join(work, 'npm-cache'),
  npm_config_update_notifier: 'false',
});
let cookie = '',
  token;
const checks = [];

async function api(method, path, body) {
  const response = await fetch(`${origin.origin}/api/v1${path}`, {
    method,
    headers: {
      'content-type': 'application/json',
      'x-donka-csrf': '1',
      ...(cookie && { cookie }),
    },
    ...(body && { body: JSON.stringify(body) }),
    signal: AbortSignal.timeout(30_000),
  });
  assert.ok(response.ok, `${method} ${path}: HTTP ${response.status}`);
  if (path === '/auth/sign-in') {
    cookie = response.headers
      .getSetCookie()
      .map((value) => value.split(';')[0])
      .join('; ');
    assert.ok(cookie, 'The temporary administrator session was issued');
  }
  return response.status === 204 ? null : response.json();
}

async function run(command, args, directory, values = {}) {
  const result = await new Promise((complete, reject) => {
    const child = spawn(command, args, {
      cwd: directory,
      env: { ...environment, ...values },
      stdio: ['ignore', 'pipe', 'pipe'],
      timeout: 120_000,
    });
    let stdout = '',
      stderr = '';
    child.stdout.on('data', (chunk) => {
      stdout += chunk.toString();
    });
    child.stderr.on('data', (chunk) => {
      stderr += chunk.toString();
    });
    child.once('error', reject);
    child.once('close', (code) => complete({ code, stdout, stderr }));
  });
  // The GitHub action registers its mask; capture the directive without printing its value.
  const output = result.stdout
    .split('\n')
    .filter((line) => !line.startsWith('::add-mask::'))
    .join('\n');
  if (token)
    assert.equal(
      output.includes(token.token) || result.stderr.includes(token.token),
      false,
      'Template/CLI output must not expose the credential',
    );
  return { ...result, output };
}

const keyValues = (text) =>
  Object.fromEntries(
    text
      .split('\n')
      .filter((line) => line.includes('='))
      .map((line) => {
        const index = line.indexOf('=');
        return [line.slice(0, index), line.slice(index + 1)];
      }),
  );
const yaml = async (path) => parse(await readFile(join(cli, path), 'utf8'));

try {
  await api('POST', '/auth/sign-in', {
    email: required('DONKA_E2E_ADMIN_EMAIL'),
    password: required('DONKA_E2E_ADMIN_PASSWORD'),
  });
  const project = await api('POST', '/projects', {
    key: `templates-${randomUUID().slice(0, 8)}`,
    name: 'Real CLI template verification',
    description: '',
  });
  const path = `/projects/${project.id}`;
  const contract = {
    type: 'object',
    required: ['input'],
    properties: {
      input: {
        type: 'integer',
        'x-donka': { label: { en: 'Input', fr: 'Entrée' } },
      },
    },
  };
  const decision = await api('POST', `${path}/decisions`, {
    key: 'retail/score',
    content: {
      nodes: [
        {
          id: 'input',
          type: 'inputNode',
          name: 'Request',
          position: { x: 0, y: 0 },
          content: { schema: JSON.stringify(contract) },
        },
        {
          id: 'calculate',
          type: 'expressionNode',
          name: 'Calculate',
          position: { x: 0, y: 160 },
          content: {
            expressions: [{ id: 'value', key: 'output', value: 'input * 2' }],
          },
        },
        {
          id: 'output',
          type: 'outputNode',
          name: 'Response',
          position: { x: 0, y: 320 },
        },
      ],
      edges: [
        { id: 'in', sourceId: 'input', targetId: 'calculate' },
        { id: 'out', sourceId: 'calculate', targetId: 'output' },
      ],
    },
  });
  await api('POST', `${path}/decisions/${decision.id}/versions`, {
    revision: decision.revision,
    message: 'Template contract',
  });
  const release = await api('POST', `${path}/releases`, {
    bump: 'minor',
    notes: 'Real service template fixture',
  });
  await api('POST', `${path}/environments/staging/deployments`, {
    releaseId: release.id,
  });
  const deadline = Date.now() + 60_000;
  let live;
  while (Date.now() < deadline) {
    const state = await api('GET', `${path}/environments`);
    live = state.items.find((item) => item.environment === 'staging')?.live;
    if (live?.releaseId === release.id) break;
    await delay(200);
  }
  assert.equal(live?.releaseId, release.id, 'The real staging publication completed');
  token = await api('POST', `${path}/ci-tokens`, {
    name: 'Disposable template reader',
  });
  token.path = `${path}/ci-tokens/${token.id}`;
  const cliEnvironment = {
    DONKA_URL: origin.origin,
    DONKA_PROJECT: project.key,
    DONKA_TARGET: 'env:staging',
    DONKA_TOKEN: token.token,
  };
  let result = await run(
    process.execPath,
    [join(cli, 'dist/main.js'), 'form', 'pull', '--out', 'contracts', '--json'],
    work,
    cliEnvironment,
  );
  assert.equal(result.code, 0, 'The built CLI pulled the actual contract');
  result = await run(
    process.execPath,
    [
      join(cli, 'dist/main.js'),
      'form',
      'init',
      '--contract',
      `contracts/${decision.key}/input.schema.json`,
      '--lang',
      'fr',
      '--out',
      'form.json',
    ],
    work,
    cliEnvironment,
  );
  assert.equal(result.code, 0, 'The built CLI initialized the template form');
  const form = join(work, 'form.json'),
    drift = join(work, 'drift.json');
  await writeFile(drift, JSON.stringify({ ...contract, properties: { input: { type: 'string' } } }));
  result = await run('npm', ['pack', '--silent', '--pack-destination', work], cli);
  assert.equal(result.code, 0, 'The CLI release tarball was packed');
  const tarball = (await readdir(work)).find((file) => file.endsWith('.tgz'));
  assert.ok(tarball, 'npm pack produced a release package');
  const packageFile = join(work, tarball);

  async function template(provider, action, matching = true, unchanged = false) {
    const directory = await mkdtemp(join(work, 'run-'));
    const runner = join(directory, 'runner');
    // A private runner directory also holds GitHub output files and Azure's temp macro.
    await mkdir(runner);
    let script, shell, env, report;
    const selectedForm = matching ? form : drift;
    if (provider === 'GitHub') {
      const definition = await yaml(`actions/${action}/action.yml`);
      const step = definition.runs.steps[0];
      const values = {
        ...Object.fromEntries(Object.entries(definition.inputs).map(([key, value]) => [key, value.default ?? ''])),
        url: origin.origin,
        project: project.key,
        target: 'env:staging',
        token: token.token,
        out: 'rules',
        decision: decision.key,
        form: selectedForm,
        current: unchanged ? live.id : '',
        'cli-package': packageFile,
      };
      env = Object.fromEntries(
        Object.entries(step.env).map(([name, value]) => {
          const input = /inputs(?:\.([\w]+)|\['([\w-]+)'\])/.exec(value);
          assert.ok(input, 'A GitHub template environment value names an input');
          return [name, String(values[input[1] ?? input[2]])];
        }),
      );
      env.GITHUB_OUTPUT = join(runner, 'outputs');
      env.RUNNER_TEMP = runner;
      await writeFile(env.GITHUB_OUTPUT, '', { mode: 0o600 });
      script = step.run;
      shell = 'bash';
      report = () => readFile(env.GITHUB_OUTPUT, 'utf8').then(keyValues);
    } else if (provider === 'GitLab') {
      const definition = (await yaml(`templates/gitlab-ci-${action}.yml`))[`.donka-${action}`];
      script = definition.script.join('\n');
      shell = 'sh';
      env = {
        ...definition.variables,
        ...cliEnvironment,
        DONKA_OUT: 'rules',
        DONKA_CURRENT: unchanged ? live.id : '',
        DONKA_DECISION: decision.key,
        DONKA_FORM: selectedForm,
        DONKA_CLI_PACKAGE: packageFile,
      };
      report = async () => {
        const values = keyValues(await readFile(join(directory, definition.artifacts.reports.dotenv), 'utf8'));
        return {
          changed: values.RULES_CHANGED,
          release: values.RULES_RELEASE,
          commit: values.RULES_COMMIT,
          sha256: values.RULES_SHA256,
        };
      };
    } else {
      const definition = await yaml(`templates/azure-pipelines-${action}.yml`);
      const step = definition.steps.find((item) => item.script);
      const values = {
        ...Object.fromEntries(definition.parameters.map((item) => [item.name, item.default ?? ''])),
        url: origin.origin,
        project: project.key,
        target: 'env:staging',
        out: 'rules',
        decision: decision.key,
        form: selectedForm,
        current: unchanged ? live.id : '',
        cliPackage: packageFile,
      };
      const expand = (text) =>
        text
          .replace(/\$\{\{ lower\(parameters\.(\w+)\) \}\}/g, (_, key) => String(values[key]).toLowerCase())
          .replace(/\$\{\{ parameters\.(\w+) \}\}/g, (_, key) => String(values[key]))
          .replaceAll('$(Agent.TempDirectory)', runner);
      assert.ok(!step.script.includes('$(DONKA_TOKEN)'), 'The Azure script never embeds a credential');
      script = expand(step.script);
      shell = 'bash';
      env = Object.fromEntries(
        Object.entries(step.env).map(([key, value]) => [key, expand(value).replaceAll('$(DONKA_TOKEN)', token.token)]),
      );
      report = async (stdout) => {
        const values = Object.fromEntries(
          [...stdout.matchAll(/##vso\[task\.setvariable variable=(\w+)\](.*)/g)].map(([, key, value]) => [key, value]),
        );
        return {
          changed: values.rulesChanged,
          release: values.rulesRelease,
          commit: values.rulesCommit,
          sha256: values.rulesSha256,
        };
      };
    }
    const result = await run(shell, ['-c', script], directory, env);
    const expected = action === 'form-check' && !matching ? 1 : 0;
    assert.equal(result.code, expected, `${provider} ${action} ${matching ? 'matching' : 'drift'} exit code`);
    if (provider === 'GitHub')
      assert.equal(
        result.stdout.includes(`::add-mask::${token.token}`),
        true,
        'The GitHub action registers its credential mask',
      );
    if (action === 'pull') {
      const values = await report(result.output);
      assert.equal(values.changed, unchanged ? 'false' : 'true', `${provider} changed output`);
      if (!unchanged) {
        assert.equal(values.release, release.id, `${provider} release output`);
        assert.equal(values.commit, live.id, `${provider} environment commit output`);
        const checksum = createHash('sha256')
          .update(await readFile(join(directory, 'rules', project.key)))
          .digest('hex');
        assert.equal(values.sha256, checksum, `${provider} checksum matches the actual downloaded file`);
      }
    } else if (!matching)
      assert.equal((result.output + result.stderr).includes('input'), true, `${provider} reports the drifted field`);
    checks.push({
      provider,
      action,
      case: action === 'pull' ? (unchanged ? 'unchanged' : 'download') : matching ? 'matching' : 'drift',
      exit: result.code,
    });
  }
  for (const provider of ['GitHub', 'GitLab', 'Azure']) {
    await template(provider, 'pull');
    await template(provider, 'pull', true, true);
    await template(provider, 'form-check');
    await template(provider, 'form-check', false);
  }
  assert.equal(checks.length, 12);
  console.log(
    JSON.stringify({
      templates: 6,
      checks: 12,
      studio: 'real',
      cli: 'release tarball',
      results: checks,
    }),
  );
} finally {
  try {
    if (token) await api('DELETE', token.path);
  } finally {
    try {
      if (cookie) await api('POST', '/auth/sign-out');
    } finally {
      assert.equal(dirname(resolve(work)), resolve(tmpdir()));
      assert.ok(basename(work).startsWith('donka-cli-templates-'));
      await rm(work, { recursive: true, force: true });
    }
  }
}
