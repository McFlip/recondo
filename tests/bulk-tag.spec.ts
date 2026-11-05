import { expect, test, type Page } from './fixtures/test';
import { randomUUID } from 'node:crypto';
import { unlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

async function seedMedia(page: Page) {
  const image = join(process.cwd(), 'ui/web/tests/fixtures/data_trap.png');
  const image2 = join(process.cwd(), 'ui/web/tests/fixtures/exploits_of_a_mom.png');
  const document = join(tmpdir(), `recondo-bulk-tag-${randomUUID()}.txt`);
  writeFileSync(document, `bulk tag test ${randomUUID()}`);

  try {
    await page.goto('/upload');
    await page.locator('#file-input').setInputFiles([image, image2, document]);
    await expect(page.locator('#upload-status')).toContainText('Upload complete');
  } finally {
    unlinkSync(document);
  }
}

test('bulk tag applies the submitted tag name to all results', async ({ page }) => {
  await seedMedia(page);

  await page.goto('/explore');
  await page.getByRole('searchbox').fill('to');
  await page.getByRole('button', { name: 'to do' }).click();
  await page.getByRole('link', { name: 'needs tags' }).click();

  await expect(page).toHaveURL(/\/media\?/);

  await page.getByRole('button', { name: 'Tag results' }).click();
  await page.getByLabel('Relationship', { exact: true }).fill('bulkrel');
  await page.getByLabel('Tag', { exact: true }).fill('bulktag');
  await page.getByRole('button', { name: 'Add to all results' }).click();

  await expect(page).toHaveURL(/notice=Tag\+added/);

  await page.goto('/explore');
  await page.getByRole('searchbox').fill('bulkrel');
  await page.getByRole('button', { name: 'bulkrel' }).click();
  await expect(page.locator('#relationship-details')).toContainText('bulktag');
});
