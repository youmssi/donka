import {
  connectorConfig,
  connectorSchema,
  connectorValues,
  presetValues,
  urlHost,
  type ConnectorValues,
} from './connector';

function valid(overrides: Partial<ConnectorValues> = {}): ConnectorValues {
  return { ...presetValues('bureau-score'), url: 'https://bureau.example/score', ...overrides };
}

function errorsOf(values: ConnectorValues): Record<string, string> {
  const result = connectorSchema.safeParse(values);
  if (result.success) return {};
  return Object.fromEntries(result.error.issues.map((issue) => [issue.path.join('.'), issue.message]));
}

it('accepts the bureau score preset once it has a URL', () => {
  expect(errorsOf(valid())).toEqual({});
  expect(errorsOf(valid({ url: '' }))).toEqual({ url: 'connectorUrl' });
});

it('checks the fields the Runtime checks', () => {
  expect(errorsOf(valid({ url: 'ftp://bureau.example' }))).toEqual({ url: 'connectorUrl' });
  expect(errorsOf(valid({ outputKey: 'bureau.score' }))).toEqual({ outputKey: 'outputKey' });
  expect(errorsOf(valid({ secret: 'bureau_key' }))).toEqual({ secret: 'secretName' });
  expect(errorsOf(valid({ secret: 'K'.repeat(65) }))).toEqual({ secret: 'secretName' });
  expect(errorsOf(valid({ header: 'X Api' }))).toEqual({ header: 'headerName' });
  expect(errorsOf(valid({ body: '{ nationalId: 1 }' }))).toEqual({ body: 'json' });
  expect(errorsOf(valid({ mock: '' }))).toEqual({ mock: 'json' });
  expect(errorsOf(valid({ timeoutMs: '20000', retries: '4' }))).toEqual({
    timeoutMs: 'wholeNumberMax',
    retries: 'wholeNumberMax',
  });
});

it('asks for a fallback only when the node falls back', () => {
  expect(errorsOf(valid({ fallback: 'nope' }))).toEqual({ fallback: 'json' });
  expect(errorsOf(valid({ onError: 'fail', fallback: 'nope' }))).toEqual({});
});

it('shows a dependent field problem alongside the others', () => {
  expect(errorsOf(valid({ url: '', secret: '' }))).toEqual({ url: 'connectorUrl', secret: 'secretName' });
});

it('stores a secret name, never more than the settings', () => {
  const config = connectorConfig(valid({ timeoutMs: '1500' }));
  expect(config).toEqual({
    preset: 'bureau-score',
    url: 'https://bureau.example/score',
    auth: { type: 'header', header: 'X-Api-Key', secret: 'BUREAU_API_KEY' },
    body: { nationalId: '{{ applicant.nationalId }}' },
    outputKey: 'bureau',
    timeoutMs: 1500,
    onError: 'fallback',
    fallback: { score: null, available: false },
    mock: { score: 712, available: true },
  });
  const plain = connectorConfig({ ...presetValues('http'), url: 'http://kyc.local/check' });
  expect(plain.auth).toEqual({ type: 'none' });
  expect(plain).not.toHaveProperty('fallback');
  expect(plain).not.toHaveProperty('retries');
});

it('reads stored settings back into the form', () => {
  const values = valid({ retries: '2', onError: 'fail', fallback: '' });
  expect(connectorValues(connectorConfig(values))).toEqual(values);
  expect(connectorValues(undefined)).toEqual({ ...presetValues('http'), url: '' });
  expect(connectorValues({ preset: 'bureau-score' })).toEqual({ ...presetValues('bureau-score'), url: '' });
});

it('shows a URL by its host', () => {
  expect(urlHost('https://api.bureau.example/v2/score')).toBe('api.bureau.example');
  expect(urlHost('not a url')).toBe('not a url');
});
