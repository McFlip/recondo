import { expect, test, type Page } from './fixtures/test';
import { randomUUID } from 'node:crypto';
import { unlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

async function seedMedia(page: Page) {
  const image = join(process.cwd(), 'ui/web/tests/fixtures/data_trap.png');
  const document = join(tmpdir(), `recondo-upload-selected-${randomUUID()}.txt`);
  writeFileSync(document, `upload selected tags test ${randomUUID()}`);

  try {
    await page.goto('/upload');
    await page.locator('#file-input').setInputFiles([image, document]);
    await expect(page.locator('#upload-status')).toContainText('Upload complete');
  } finally {
    unlinkSync(document);
  }
}

test('uploads with tags selected on the media page', async ({ page }) => {
  await seedMedia(page);

  await page.goto('/explore');
  await page.getByRole('searchbox').fill('to');
  await page.getByRole('button', { name: 'to do' }).click();
  await page.getByRole('link', { name: 'needs tags' }).click();

  await expect(page).toHaveURL(/\/media\?tag=/);
  await page.getByRole('button', { name: 'Upload with selected tags' }).click();

  await expect(page).toHaveURL(/\/upload/);
  await expect(page.locator('.selected-upload-tag')).toHaveCount(1);
  await expect(page.locator('.selected-upload-tag')).toContainText('#needs tags · to do');

  const file = join(tmpdir(), `recondo-upload-selected-file-${randomUUID()}.txt`);
  writeFileSync(file, `uploaded with selected tags ${randomUUID()}`);
  try {
    await page.locator('#file-input').setInputFiles(file);
    await expect(page.locator('#upload-status')).toContainText('Upload complete');
    await expect(page.locator('.processed-tags')).toContainText('#needs tags · to do');
  } finally {
    unlinkSync(file);
  }
});
