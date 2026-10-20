import assert from 'node:assert/strict';
import { readFile, stat } from 'node:fs/promises';
import { createServer } from 'node:http';
import { dirname, extname, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium, expect } from '@playwright/test';

const webDir = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const root = resolve(process.env.DONKA_STORYBOOK_DIR ?? resolve(webDir, 'storybook-static'));
const mime = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.json': 'application/json',
  '.svg': 'image/svg+xml',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.ttf': 'font/ttf',
  '.png': 'image/png',
  '.ico': 'image/x-icon',
};
await stat(resolve(root, 'index.json')).catch((cause) => {
  throw new Error('Build Storybook first, or set DONKA_STORYBOOK_DIR to its build directory.', { cause });
});

// Serve only the built preview, on a private ephemeral port; no development server is needed.
const server = createServer(async (request, response) => {
  try {
    if (request.method !== 'GET' && request.method !== 'HEAD') {
      response.writeHead(405).end();
      return;
    }
    const pathname = decodeURIComponent(new URL(request.url ?? '/', 'http://localhost').pathname);
    let target = resolve(root, `.${pathname}`);
    if (target !== root && !target.startsWith(`${root}${sep}`)) {
      response.writeHead(403).end();
      return;
    }
    if ((await stat(target)).isDirectory()) target = resolve(target, 'index.html');
    const data = await readFile(target);
    const extension = extname(target);
    response.writeHead(200, {
      'content-type': mime[extension] ?? 'application/octet-stream',
      'cache-control': extension === '.html' || extension === '.json' ? 'no-store' : 'public, max-age=3600',
    });
    response.end(request.method === 'HEAD' ? undefined : data);
  } catch (error) {
    if (error.code === 'ENOENT' || error.code === 'ENOTDIR') response.writeHead(404).end();
    else if (error instanceof URIError) response.writeHead(400).end();
    else {
      console.error('Storybook static server:', error);
      response.writeHead(500).end();
    }
  }
});
await new Promise((done) => server.listen(0, '127.0.0.1', done));
const address = server.address();
assert.ok(address && typeof address !== 'string');
const base = `http://127.0.0.1:${address.port}`;
let browser;

async function interactions(page, entry, m, theme, width) {
  if (entry.title === 'Shared/Password' && entry.name === 'Default') {
    const password = page.getByLabel(m.signIn.password, { exact: true });
    const confirm = page.getByLabel(m.setupPassword.confirm, { exact: true });
    await expect(password).toHaveAttribute('type', 'password');
    await expect(confirm).toHaveAttribute('type', 'password');
    await password.fill('preview verification phrase');
    await page.getByRole('button', { name: `${m.common.showPassword}: ${m.signIn.password}`, exact: true }).click();
    await expect(password).toHaveAttribute('type', 'text');
    await expect(confirm).toHaveAttribute('type', 'password');
    await expect(password).toHaveValue('preview verification phrase');
    await page.getByRole('button', { name: `${m.common.hidePassword}: ${m.signIn.password}`, exact: true }).click();
    await expect(password).toHaveAttribute('type', 'password');
    await expect(page.getByRole('alert')).toHaveCount(0);
  }
  if (entry.title === 'UI/Content' && entry.name === 'Copy Failure') {
    await page.getByRole('button', { name: m.common.copyToken, exact: true }).click();
    await expect(page.getByText(m.common.tokenCopyFailed, { exact: true })).toBeVisible();
  }
  if (entry.title === 'Shared/Form' && entry.name === 'Loading') {
    await expect(page.getByRole('button').filter({ hasText: m.common.saving })).toBeDisabled();
  }
  if (entry.title === 'Shared/Form' && entry.name === 'Error State') {
    await expect(page.locator('[aria-invalid="true"]')).toHaveCount(2);
  }
  if (entry.title === 'Shared/Feedback' && entry.name === 'Error State') {
    await expect(page.getByRole('alert')).toContainText('preview-reference');
  }
  if (entry.title === 'UI/Overlays' && entry.name === 'Dialog View') {
    await page.getByRole('button', { name: m.projects.create, exact: true }).click();
    await expect(page.getByRole('dialog')).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(page.getByRole('dialog')).toHaveCount(0);
  }
  if (entry.title === 'UI/Overlays' && entry.name === 'Confirmation') {
    await page.getByRole('button', { name: m.project.archive, exact: true }).click();
    await expect(page.getByRole('alertdialog')).toBeVisible();
    await page.getByRole('button', { name: m.common.cancel, exact: true }).click();
    await expect(page.getByRole('alertdialog')).toHaveCount(0);
  }
  if (entry.title === 'UI/Overlays' && entry.name === 'Side Sheet') {
    await page.getByRole('button', { name: m.inputFields.open, exact: true }).click();
    await expect(page.getByRole('dialog')).toContainText(m.inputFields.title);
    await page.keyboard.press('Escape');
    await expect(page.getByRole('dialog')).toHaveCount(0);
  }
  if (entry.title === 'UI/Overlays' && entry.name === 'Popover View') {
    await page.getByRole('button', { name: m.projects.description, exact: true }).click();
    await expect(page.getByRole('dialog')).toContainText(m.projects.emptyAdmin);
    await page.keyboard.press('Escape');
    await expect(page.getByRole('dialog')).toHaveCount(0);
  }
  if (entry.title === 'UI/Overlays' && entry.name === 'Tooltip View') {
    await page.getByRole('button', { name: m.common.theme, exact: true }).focus();
    await expect(page.getByRole('tooltip')).toContainText(m.common.themeSystem);
    await page.keyboard.press('Escape');
    await expect(page.getByRole('tooltip')).toHaveCount(0);
  }
  if (entry.title === 'Shared/Dates' && ['Empty Range', 'Selected Range'].includes(entry.name)) {
    await page.getByRole('button', { name: m.decisionLog.dates, exact: true }).click();
    await expect(page.getByRole('dialog').getByRole('grid')).toHaveCount(2);
    await page.keyboard.press('Escape');
    await expect(page.getByRole('dialog')).toHaveCount(0);
  }
  if (entry.title === 'Shared/Form' && entry.name === 'Empty State') {
    await page.getByLabel(m.projects.name, { exact: true }).fill(m.tokens.namePlaceholder);
    await page.getByRole('combobox', { name: m.members.role, exact: true }).click();
    await page.getByRole('option', { name: m.roles.editor, exact: true }).click();
    await page.getByRole('checkbox', { name: m.inputFields.required, exact: true }).uncheck();
    await page.getByRole('button', { name: m.common.save, exact: true }).click();
    await expect(page.getByRole('alert')).toHaveText(m.project.saved);
    await expect(page.locator('[aria-invalid="true"]')).toHaveCount(0);
  }
  if (entry.title === 'UI/Actions' && entry.name === 'Radio Choices') {
    await page.getByRole('radio', { name: m.common.themeDark, exact: true }).check();
    await expect(page.getByRole('radio', { name: m.common.themeDark, exact: true })).toBeChecked();
    await expect(page.getByRole('radio', { name: m.common.themeSystem, exact: true })).toBeDisabled();
  }
  if (entry.title === 'UI/Actions' && entry.name === 'Toasts') {
    await page.getByRole('button', { name: m.common.save, exact: true }).click();
    await expect(page.getByText(m.project.saved, { exact: true })).toBeVisible();
    await page.getByRole('button', { name: m.common.retry, exact: true }).click();
    await expect(page.getByText(m.errors.NETWORK, { exact: true })).toBeVisible();
  }
  if (entry.title === 'UI/Navigation' && entry.name === 'Row Menu') {
    await page.getByRole('button', { name: m.common.actions, exact: true }).click();
    await expect(page.getByRole('menu')).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(page.getByRole('menu')).toHaveCount(0);
  }
  if (entry.title === 'UI/Navigation' && entry.name === 'Tabs And Breadcrumb') {
    await page.getByRole('tab', { name: m.members.title, exact: true }).click();
    await expect(page.getByRole('tabpanel')).toHaveText(m.members.readOnly);
  }
  if (entry.title === 'UI/Sidebar' && entry.name === 'Success' && width === 390) {
    await page.getByRole('button', { name: m.ui.toggleSidebar, exact: true }).click();
    await expect(page.getByRole('dialog')).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(page.getByRole('dialog')).toHaveCount(0);
  }
  if (entry.title === 'Shared/Tour') {
    await page.getByRole('button', { name: m.help.tour, exact: true }).click();
    await expect(page.locator('.driver-popover')).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(page.locator('.driver-popover')).toHaveCount(0);
  }
  if (entry.title === 'Shared/Layout' && entry.name === 'Branding And Preferences') {
    const alternate = theme === 'dark' ? 'light' : 'dark';
    for (const selected of [alternate, theme]) {
      await page.getByRole('button', { name: m.common.theme, exact: true }).click();
      await page
        .getByRole('menuitemradio', {
          name: selected === 'dark' ? m.common.themeDark : m.common.themeLight,
          exact: true,
        })
        .click();
      await expect
        .poll(() => page.locator('html').evaluate((html) => html.classList.contains('dark')))
        .toBe(selected === 'dark');
    }
  }
}

try {
  const index = await (await fetch(`${base}/index.json`)).json();
  const entries = Object.values(index.entries).filter((entry) => entry.type === 'story');
  assert.ok(entries.length > 0, 'Storybook must contain component stories');
  browser = await chromium.launch({ headless: true, channel: process.env.DONKA_TEST_BROWSER_CHANNEL || undefined });
  const context = await browser.newContext();
  const page = await context.newPage();
  page.setDefaultTimeout(15_000);
  page.setDefaultNavigationTimeout(30_000);
  let errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  page.on('console', (message) => {
    if (message.type() === 'error') errors.push(`${message.text()} (${message.location().url})`);
  });
  let checked = 0;
  for (const locale of ['en', 'fr']) {
    const m = JSON.parse(await readFile(resolve(webDir, `src/messages/${locale}.json`), 'utf8'));
    for (const theme of ['light', 'dark']) {
      await page.emulateMedia({ colorScheme: theme });
      for (const viewport of [
        { width: 1280, height: 900 },
        { width: 390, height: 844 },
      ]) {
        await page.setViewportSize(viewport);
        for (const entry of entries) {
          errors = [];
          try {
            const query = new URLSearchParams({
              id: entry.id,
              viewMode: 'story',
              globals: `locale:${locale};theme:${theme}`,
            });
            await page.goto(`${base}/iframe.html?${query}`);
            await expect(page.locator('#storybook-root')).toBeVisible();
            await expect(page.locator('#storybook-root')).not.toBeEmpty();
            await expect(page.locator('#error-message')).not.toBeVisible();
            await expect(page.locator('html')).toHaveAttribute('lang', locale);
            await expect
              .poll(() => page.locator('html').evaluate((html) => html.classList.contains('dark')))
              .toBe(theme === 'dark');
            await page.evaluate(() => document.fonts.ready);
            await interactions(page, entry, m, theme, viewport.width);
            assert.ok(
              await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth),
              'No document-wide horizontal overflow',
            );
            assert.deepEqual(errors, [], 'No browser or render errors');
            checked++;
          } catch (error) {
            throw new Error(`${entry.id} (${locale}, ${theme}, ${viewport.width}px)`, { cause: error });
          }
        }
        console.log(`Storybook: ${entries.length} stories passed ${locale}/${theme}/${viewport.width}px`);
      }
    }
  }
  console.log(
    `Storybook: ${checked} render checks passed, including password, validation, overlay, theme and tour interactions.`,
  );
} finally {
  try {
    await browser?.close();
  } finally {
    await new Promise((done) => server.close(done));
  }
}
