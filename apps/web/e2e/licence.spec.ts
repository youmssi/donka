import { test, expect } from './fixtures';
import { openPage } from './navigation';

// The live stack runs on a licence issued with the development key (scripts/dev-licence.sh),
// which only a development build trusts (DNK-77).
test('the installation runs on its verified licence, with every module and no notice far from the end', async ({
  admin,
  context,
  messages: m,
  appLocale,
}) => {
  const response = await admin.get('/api/v1/licence');
  await expect(response).toBeOK();
  expect(await response.json()).toMatchObject({
    status: 'active',
    customer: 'Donka development',
    expiring: false,
    allowsChanges: true,
    allowsProduction: true,
    modules: ['explain', 'connectors', 'analysis'],
  });

  const page = await openPage(context, `/${appLocale}/`);
  await expect(page.getByRole('main')).toBeVisible();
  for (const title of [m.licence.evaluationTitle, m.licence.refusedTitle]) {
    await expect(page.getByText(title, { exact: true })).toHaveCount(0);
  }
});
