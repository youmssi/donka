import { conflictOf, createDecisionSchema, parseObject, scenarioSchema, splitKey } from './schema';

it('splits a key into its folder and name', () => {
  expect(splitKey('bureau/normalize')).toEqual({ folder: 'bureau/', name: 'normalize' });
  expect(splitKey('v2/limits/card')).toEqual({ folder: 'v2/limits/', name: 'card' });
  expect(splitKey('person-score')).toEqual({ folder: '', name: 'person-score' });
});

it('accepts the keys the API accepts', () => {
  for (const key of ['person-score', 'bureau/normalize', 'a1/b-2']) {
    expect(createDecisionSchema.safeParse({ key }).success).toBe(true);
  }
  for (const key of ['', 'Person', 'a//b', 'bureau/', '-x', 'a b']) {
    expect(createDecisionSchema.safeParse({ key }).success).toBe(false);
  }
});

it('reads who saved first from a conflict', () => {
  const details = {
    revision: 4,
    updatedAt: '2026-09-29T10:00:00Z',
    updatedBy: { id: 'u-2', email: 'grace@bank.example' },
  };
  expect(conflictOf(details)).toEqual(details);
  expect(conflictOf({ revision: 4, updatedAt: '2026-09-29T10:00:00Z', updatedBy: null })?.updatedBy).toBeNull();
  expect(conflictOf(undefined)).toBeNull();
  expect(conflictOf({ nope: true })).toBeNull();
});

it('takes JSON objects only for scenario input and expected output', () => {
  expect(parseObject('{"income": 150000}')).toEqual({ income: 150000 });
  for (const bad of ['[1]', '10', 'null', '{', '']) expect(parseObject(bad)).toBeNull();
  const valid = { decisionId: 'd-1', name: ' Big ', input: '{}', expected: '{"ok": true}', match: 'exact' };
  expect(scenarioSchema.safeParse(valid).success).toBe(true);
  expect(scenarioSchema.safeParse({ ...valid, input: '[]' }).success).toBe(false);
  expect(scenarioSchema.safeParse({ ...valid, name: '  ' }).success).toBe(false);
});
