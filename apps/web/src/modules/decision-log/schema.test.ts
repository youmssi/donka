import { duration, isEmptyRange, readFilters, settingsSchema, toQuery } from './schema';

it('reads the filters from the address and drops what the API would refuse', () => {
  const params = new URLSearchParams({
    reference: ' APP-1 ',
    decision: 'person-score',
    outcome: 'approve',
    environment: 'production',
    status: 'failed',
    from: '2026-09-01',
    to: 'yesterday',
  });
  expect(readFilters(params)).toEqual({
    reference: 'APP-1',
    decision: 'person-score',
    outcome: 'approve',
    environment: 'production',
    status: 'failed',
    from: '2026-09-01',
    to: undefined,
  });
  expect(readFilters(new URLSearchParams({ environment: 'qa', status: 'maybe' }))).toEqual({
    reference: undefined,
    decision: undefined,
    outcome: undefined,
    environment: undefined,
    status: undefined,
    from: undefined,
    to: undefined,
  });
});

it('asks the API for whole days, the last one included', () => {
  const query = toQuery({ decision: 'limit', from: '2026-09-01', to: '2026-09-02' });
  expect(query).toEqual({
    reference: undefined,
    decisionKey: 'limit',
    outcome: undefined,
    environment: undefined,
    status: undefined,
    from: new Date(2026, 8, 1).toISOString(),
    until: new Date(2026, 8, 3).toISOString(),
  });
  expect(isEmptyRange({ from: '2026-09-02', to: '2026-09-01' })).toBe(true);
});

it('shows durations in the unit that reads best', () => {
  expect(duration(420, 'en')).toBe('420 µs');
  expect(duration(1840, 'en')).toBe('1.8 ms');
  expect(duration(1840, 'fr')).toBe('1,8 ms');
});

it('takes an outcome field as dotted names, or nothing', () => {
  for (const good of ['', 'decision', 'result.band'])
    expect(settingsSchema.safeParse({ outcomeField: good }).success).toBe(true);
  for (const bad of ['1st', 'a-b', 'a..b']) expect(settingsSchema.safeParse({ outcomeField: bad }).success).toBe(false);
});
