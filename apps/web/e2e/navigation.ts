import { expect, type BrowserContext, type Page } from '@playwright/test';

/** Cold foreground navigation keeps every page observable without replacing a departing document. */
export async function openPage(context: BrowserContext, url: string, previous?: Page): Promise<Page> {
  const page = await context.newPage();
  await page.bringToFront();
  await previous?.close();
  await page.goto(url);
  await expect.poll(() => page.evaluate(() => document.visibilityState)).toBe('visible');
  return page;
}
