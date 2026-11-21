import { createProjectSchema, keyFromName, KEY_RULES, ROLES } from './schema';

describe('project keys', () => {
  it('are proposed from the name', () => {
    expect(keyFromName('Crédit PME 2026')).toBe('credit-pme-2026');
    expect(keyFromName('  Retail   scoring!! ')).toBe('retail-scoring');
    expect(keyFromName('2026 budget')).toBe('budget');
    expect(keyFromName('x'.repeat(60))).toHaveLength(KEY_RULES.max);
  });

  // The same cases as the Rust check in crates/project: the contract's pattern agrees with it.
  it.each(['credit-scoring', 'sme2', 'ab', 'a-1-b'])('accepts %s', (key) => {
    expect(createProjectSchema.safeParse({ key, name: 'x', description: '' }).success).toBe(true);
  });

  it.each([
    'a',
    'Credit',
    '1credit',
    'credit_scoring',
    'credit--scoring',
    'credit-',
    '-credit',
    'crédit',
    'a'.repeat(41),
  ])('refuses %s', (key) => {
    expect(createProjectSchema.safeParse({ key, name: 'x', description: '' }).success).toBe(false);
  });
});

it('lists roles from the most powerful', () => {
  expect(ROLES).toEqual(['owner', 'editor', 'viewer']);
});
