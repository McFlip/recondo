import { expect, test, type Page } from './fixtures/test';
import { randomUUID } from 'node:crypto';
import { unlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

async function seedMedia(page: Page) {
  const image = join(process.cwd(), 'ui/web/tests/fixtures/data_trap.png');
  const document = join(tmpdir(), `recondo-delete-tag-${randomUUID()}.txt`);
  writeFileSync(document, `delete tag test ${randomUUID()}`);

  try {
    await page.goto('/upload');
    await page.locator('#file-input').setInputFiles([image, document]);
    await expect(page.locator('#upload-status')).toContainText('Upload complete');
  } finally {
    unlinkSync(document);
  }
}

test('delete tag removes it from all results', async ({ page }) => {
  await seedMedia(page);
  await page.goto('/explore');
  await page.getByRole('searchbox').fill('to');
  await page.getByRole('button', { name: 'to do' }).click();
  await page.getByRole('link', { name: 'needs tags' }).click();

  await page.getByRole('button', { name: 'Delete tag from results' }).click();
  await page.getByLabel('Tag to delete').selectOption({ label: 'needs tags' });
  await page.getByRole('button', { name: 'Delete', exact: true }).click();

  await expect(page).toHaveURL(/notice=Tag\+removed/);
});
