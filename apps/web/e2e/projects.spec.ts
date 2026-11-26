import type { ApiSchemas } from '../src/components/shared/api';
import { test, expect, passwordLink, unique } from './fixtures';
import { openPage } from './navigation';

test('owners invite members, change every role, preserve the last owner and remove access', async ({
  context,
  admin,
  services,
  messages: m,
  appLocale,
}, testInfo) => {
  const key = unique('membership');
  const created = await admin.post('/api/v1/projects', {
    data: { key, name: 'Membership verification', description: '' },
  });
  await expect(created).toBeOK();
  const project: ApiSchemas['ProjectResponse'] = await created.json();
  const path = `/api/v1/projects/${project.id}`;
  const meResponse = await admin.get('/api/v1/auth/me');
  await expect(meResponse).toBeOK();
  const me: ApiSchemas['UserResponse'] = await meResponse.json();
  let page = await openPage(context, `/${appLocale}/projects/members?p=${key}`);
  await page.getByRole('button', { name: m.members.actionsFor.replace('{email}', me.email), exact: true }).click();
  await page.getByRole('menuitemradio', { name: m.roles.viewer, exact: true }).click();
  await expect(page.getByText(m.errors.LAST_OWNER, { exact: true })).toBeVisible();
  const email = `${unique('project-member')}@donka.local`;
  await page.getByRole('button', { name: m.members.addTitle, exact: true }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByLabel(m.members.email, { exact: true }).fill(email);
  await dialog.getByRole('button', { name: m.members.add, exact: true }).click();
  await dialog.getByRole('button', { name: m.members.inviteAndAdd.replace('{email}', email), exact: true }).click();
  await expect(dialog).toBeHidden();
  await passwordLink(services, email);
  const row = page.getByRole('row').filter({ hasText: email });
  await expect(row.getByText(m.roles.viewer, { exact: true })).toBeVisible();
  for (const role of ['editor', 'owner', 'viewer'] as const) {
    await row.getByRole('button', { name: m.members.actionsFor.replace('{email}', email), exact: true }).click();
    await page.getByRole('menuitemradio', { name: m.roles[role], exact: true }).click();
    await expect(row.getByText(m.roles[role], { exact: true })).toBeVisible();
    await expect
      .poll(async () => {
        const response = await admin.get(`${path}/members`);
        await expect(response).toBeOK();
        const data: ApiSchemas['MemberListResponse'] = await response.json();
        return data.items.find((member) => member.email === email)?.role;
      })
      .toBe(role);
  }
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await testInfo.attach('project-members', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  });
  await row.getByRole('button', { name: m.members.actionsFor.replace('{email}', email), exact: true }).click();
  await page.getByRole('menuitem', { name: m.members.remove, exact: true }).click();
  await page.getByRole('alertdialog').getByRole('button', { name: m.common.cancel, exact: true }).click();
  await expect(row).toBeVisible();
  await row.getByRole('button', { name: m.members.actionsFor.replace('{email}', email), exact: true }).click();
  await page.getByRole('menuitem', { name: m.members.remove, exact: true }).click();
  await page.getByRole('alertdialog').getByRole('button', { name: m.members.remove, exact: true }).click();
  await expect(row).toHaveCount(0);
  page = await openPage(context, `/${appLocale}/projects/settings?p=${key}`, page);
  const renamed = unique('Renamed project');
  await page.getByLabel(m.projects.name, { exact: true }).fill(renamed);
  await page.getByLabel(m.projects.description, { exact: true }).fill('Persisted project settings');
  await page.getByRole('button', { name: m.common.save, exact: true }).first().click();
  await expect.poll(async () => (await (await admin.get(path)).json()).name).toBe(renamed);
  page = await openPage(context, `/${appLocale}/projects/settings?p=${key}`, page);
  await expect(page.getByLabel(m.projects.name, { exact: true })).toHaveValue(renamed);
  await expect(page.getByLabel(m.projects.description, { exact: true })).toHaveValue('Persisted project settings');
  await page.getByRole('button', { name: m.project.archive, exact: true }).click();
  await page.getByRole('alertdialog').getByRole('button', { name: m.common.cancel, exact: true }).click();
  await expect(page.getByLabel(m.projects.name, { exact: true })).toBeEnabled();
  await page.getByRole('button', { name: m.project.archive, exact: true }).click();
  await page.getByRole('alertdialog').getByRole('button', { name: m.project.archive, exact: true }).click();
  await expect(page.getByText(m.project.archivedNotice, { exact: true })).toBeVisible();
  await expect(page.getByLabel(m.projects.name, { exact: true })).toBeDisabled();
  const denied = await admin.patch(path, { data: { name: 'Must not persist' } });
  expect(denied.status()).toBe(409);
  expect((await denied.json()).code).toBe('PROJECT_ARCHIVED');
  const active: ApiSchemas['ProjectListResponse'] = await (await admin.get('/api/v1/projects?limit=100')).json();
  expect(active.items.some((item) => item.id === project.id)).toBe(false);
  await page.getByRole('button', { name: m.project.restore, exact: true }).click();
  await expect(page.getByText(m.project.archivedNotice, { exact: true })).toHaveCount(0);
  await expect(page.getByLabel(m.projects.name, { exact: true })).toBeEnabled();
  const restored: ApiSchemas['ProjectResponse'] = await (await admin.get(path)).json();
  expect(restored).toMatchObject({ key, name: renamed, archivedAt: null });
});

test('viewer and editor permissions, account administration and another project are enforced by the server', async ({
  context,
  admin,
  services,
  messages: m,
  appLocale,
}) => {
  const email = `${unique('permission-user')}@donka.local`;
  const password = 'disposable project permission passphrase';
  await expect(
    await admin.post('/api/v1/users/invitations', { data: { email, locale: appLocale, isAdmin: false } }),
  ).toBeOK();
  await expect(
    await admin.post('/api/v1/auth/password-setup', {
      data: { token: await passwordLink(services, email), password },
    }),
  ).toBeOK();
  const key = unique('permissions');
  const created = await admin.post('/api/v1/projects', { data: { key, name: 'Permission verification' } });
  await expect(created).toBeOK();
  const project: ApiSchemas['ProjectResponse'] = await created.json();
  const path = `/api/v1/projects/${project.id}`;
  const added = await admin.post(`${path}/members`, { data: { email, role: 'viewer' } });
  await expect(added).toBeOK();
  const member: ApiSchemas['MemberResponse'] = await added.json();
  const foreignResponse = await admin.post('/api/v1/projects', {
    data: { key: unique('other-project'), name: 'Private other project' },
  });
  await expect(foreignResponse).toBeOK();
  const foreign: ApiSchemas['ProjectResponse'] = await foreignResponse.json();
  // Replace the admin cookie with the fixture user's session; mutations still go to the real server.
  await context.clearCookies();
  await expect(
    await context.request.post('/api/v1/auth/sign-in', {
      headers: { 'x-donka-csrf': '1' },
      data: { email, password },
    }),
  ).toBeOK();
  let page = await openPage(context, `/${appLocale}/projects/members?p=${key}`);
  await expect(page.getByText(m.members.readOnly, { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: m.members.addTitle, exact: true })).toHaveCount(0);
  page = await openPage(context, `/${appLocale}/projects/settings?p=${key}`, page);
  await expect(page.getByLabel(m.projects.name, { exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: m.project.archive, exact: true })).toHaveCount(0);
  for (const role of ['viewer', 'editor'] as const) {
    await expect(await admin.patch(`${path}/members/${member.userId}`, { data: { role } })).toBeOK();
    for (const [method, suffix, data] of [
      ['PATCH', '', { name: 'Forbidden rename' }],
      ['POST', '/archive', {}],
      ['POST', '/members', { email, role: 'owner' }],
      ['PATCH', `/members/${member.userId}`, { role: 'owner' }],
      ['DELETE', `/members/${member.userId}`, {}],
    ] as const) {
      const reply = await context.request.fetch(`${path}${suffix}`, { method, data, headers: { 'x-donka-csrf': '1' } });
      expect(reply.status()).toBe(403);
      expect((await reply.json()).code).toBe('FORBIDDEN');
    }
    const decision = await context.request.post(`${path}/decisions`, {
      headers: { 'x-donka-csrf': '1' },
      data: { key: unique('permitted-edit') },
    });
    expect(decision.status()).toBe(role === 'viewer' ? 403 : 201);
  }
  for (const method of ['GET', 'PATCH'] as const) {
    const denied = await context.request.fetch(`/api/v1/projects/${foreign.id}`, {
      method,
      ...(method === 'PATCH' ? { data: { name: 'Must stay private' } } : {}),
      headers: { 'x-donka-csrf': '1' },
    });
    expect(denied.status()).toBe(404);
    expect((await denied.json()).code).toBe('PROJECT_NOT_FOUND');
  }
  page = await openPage(context, `/${appLocale}/projects/settings?p=${foreign.key}`, page);
  await expect(page.getByText(m.project.notFoundTitle, { exact: true })).toBeVisible();
  await expect(page.getByText(foreign.name, { exact: true })).toHaveCount(0);
  page = await openPage(context, `/${appLocale}/people`, page);
  await expect(page.getByText(m.people.forbiddenTitle, { exact: true })).toBeVisible();
  expect((await context.request.get('/api/v1/users')).status()).toBe(403);
  expect(
    (
      await context.request.post('/api/v1/users/invitations', {
        headers: { 'x-donka-csrf': '1' },
        data: { email: `${unique('denied-invite')}@donka.local`, locale: appLocale, isAdmin: true },
      })
    ).status(),
  ).toBe(403);
  expect(
    (
      await context.request.post('/api/v1/projects', {
        headers: { 'x-donka-csrf': '1' },
        data: { key: unique('denied-create'), name: 'Forbidden' },
      })
    ).status(),
  ).toBe(403);
  await expect(await admin.delete(`${path}/members/${member.userId}`)).toBeOK();
  expect((await context.request.get(path)).status()).toBe(404);
  await expect(await context.request.post('/api/v1/auth/sign-out', { headers: { 'x-donka-csrf': '1' } })).toBeOK();
});
