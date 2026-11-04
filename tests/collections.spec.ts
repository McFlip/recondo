import { expect, test, type Page } from './fixtures/test';
import { randomUUID } from 'node:crypto';
import { unlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

async function seedMedia(page: Page) {
  const image = join(process.cwd(), 'ui/web/tests/fixtures/data_trap.png');
  const document = join(tmpdir(), `recondo-collections-${randomUUID()}.txt`);
  writeFileSync(document, `collections test ${randomUUID()}`);

  try {
    await page.goto('/upload');
    await page.locator('#file-input').setInputFiles([image, document]);
    await expect(page.locator('#upload-status')).toContainText('Upload complete');
  } finally {
    unlinkSync(document);
  }
}

test('saves, lists, searches, views, and deletes a collection', async ({ page }) => {
  await seedMedia(page);
  await page.goto('/explore');
  await page.getByRole('searchbox').fill('to');
  await page.getByRole('button', { name: 'to do' }).click();
  await page.getByRole('link', { name: 'needs tags' }).click();

  await expect(page).toHaveURL(/\/media\?tag=/);

  await page.getByRole('button', { name: 'Save to collection' }).click();
  const name = `col ${randomUUID().slice(0, 8)}`;
  await page.getByLabel('Collection name').fill(name);
  await page.getByRole('button', { name: 'Save', exact: true }).click();

  await expect(page).toHaveURL(/\/collections/);
  await expect(page.locator('.collection-card')).toContainText(name);
  await expect(page.locator('.collection-card')).toContainText('AND');
  await expect(page.locator('.collection-card .tag-chip')).toContainText('needs tags');

  await page.getByRole('searchbox').fill(name.slice(0, 4));
  await expect(page.locator('.collection-card')).toContainText(name);

  await page.getByRole('button', { name: 'List all' }).click();
  await expect(page.locator('.collection-card')).toContainText(name);

  await page.locator('.collection-card a', { hasText: 'View' }).click();
  await expect(page).toHaveURL(/\/media\?tag=/);

  await page.goto('/collections');
  await page.locator('.collection-card button[aria-label="Delete collection"]').click();
  await expect(page.locator('.collection-card')).toHaveCount(0);
});