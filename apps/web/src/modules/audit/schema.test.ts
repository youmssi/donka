import { detail, isEmptyRange, PROJECT_ACTIONS, readFilters, toQuery, type AuditEvent } from './schema';

it('offers only the actions a project log holds', () => {
  expect(PROJECT_ACTIONS).toContain('member.role_changed');
  expect(PROJECT_ACTIONS).toContain('project.created');
  expect(PROJECT_ACTIONS.some((action) => action.startsWith('user.'))).toBe(false);
});

it('turns days in the viewer time zone into an inclusive range of instants', () => {
  expect(toQuery({ from: '2026-09-01', to: '2026-09-30' })).toEqual({
    actor: undefined,
    action: undefined,
    from: new Date(2026, 8, 1).toISOString(),
    until: new Date(2026, 9, 1).toISOString(),
  });
});

it('drops filters the API would refuse', () => {
  const filters = readFilters(new URLSearchParams('actor=u-2&action=project.exploded&from=yesterday&to=2026-09-30'));
  expect(filters).toEqual({ actor: 'u-2', action: undefined, from: undefined, to: '2026-09-30' });
});

it('knows a range ending before it starts matches nothing', () => {
  expect(isEmptyRange({ from: '2026-09-30', to: '2026-09-01' })).toBe(true);
  expect(isEmptyRange({ from: '2026-09-30', to: '2026-09-30' })).toBe(false);
  expect(isEmptyRange({ to: '2026-09-01' })).toBe(false);
});

it('reads text and number details and ignores anything else', () => {
  const event = { details: { from: { name: 'Retail' }, locked: true, version: 3 } } as unknown as AuditEvent;
  expect(detail(event, 'from', 'name')).toBe('Retail');
  expect(detail(event, 'version')).toBe('3');
  expect(detail(event, 'locked')).toBe('');
  expect(detail(event, 'to', 'name')).toBe('');
});
