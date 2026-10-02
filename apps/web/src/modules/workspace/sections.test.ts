import { placeOf, projectSections } from './sections';

it('knows where the person is from the address', () => {
  expect(placeOf('/')).toEqual({ kind: 'projects' });
  expect(placeOf('/people')).toEqual({ kind: 'people' });
  expect(placeOf('/projects/audit')).toEqual({ kind: 'project', section: 'audit' });
  expect(placeOf('/projects/members/')).toEqual({ kind: 'project', section: 'members' });
  expect(placeOf('/projects/decision')).toEqual({ kind: 'project', section: 'decision' });
  expect(placeOf('/projects/approval/')).toEqual({ kind: 'project', section: 'approval' });
  expect(placeOf('/sign-in')).toEqual({ kind: 'other' });
});

it('offers the audit log to owners only', () => {
  const project = {
    id: 'p-1',
    key: 'retail',
    name: 'Retail',
    description: '',
    createdAt: '2026-09-28T09:00:00Z',
    archivedAt: null,
  };
  const sections = (role: 'owner' | 'editor') => projectSections({ ...project, role }).map(({ section }) => section);
  expect(sections('owner')).toEqual([
    'decisions',
    'scenarios',
    'releases',
    'environments',
    'approvals',
    'members',
    'settings',
    'audit',
  ]);
  expect(sections('editor')).toEqual([
    'decisions',
    'scenarios',
    'releases',
    'environments',
    'approvals',
    'members',
    'settings',
  ]);
});
