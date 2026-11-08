import { expect, test } from './fixtures/test';
import { randomUUID } from 'node:crypto';
import { unlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

async function seedMedia(page) {
  const image = join(process.cwd(), 'ui/web/tests/fixtures/data_trap.png');
  const document = join(tmpdir(), `recondo-media-${randomUUID()}.txt`);
  writeFileSync(document, `media page test ${randomUUID()}`);
  try {
    await page.goto('/upload');
    await page.locator('#file-input').setInputFiles([image, document]);
    await expect(page.locator('#upload-status')).toContainText('Upload complete');
  } finally {
    unlinkSync(document);
  }
}

test('scroll check', async ({ page }) => {
  await seedMedia(page);
  await page.goto('/explore');
  await page.getByRole('searchbox').fill('to');
  await page.getByRole('button', { name: 'to do' }).click();
  await page.getByRole('link', { name: 'needs tags' }).click();
  await page.getByLabel('DISPLAY LIMIT').selectOption('400');
  await page.waitForLoadState('networkidle');
  await page.evaluate(() => window.scrollTo(0, 500));
  await page.waitForTimeout(200);
  const before = await page.evaluate(() => window.scrollY);
  console.log('before', before);
  await page.locator('.media-preview img').first().click();
  await page.waitForTimeout(300);
  const after = await page.evaluate(() => window.scrollY);
  console.log('after', after);
  expect(after).toBe(before);
  await expect(page.locator('#media-lightbox')).toBeVisible();
});
