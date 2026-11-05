import { expect, test, type Page } from './fixtures/test';
import { randomUUID } from 'node:crypto';
import { unlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

test('aliasing tags mirrors references', async ({ page }) => {
  const file = join(tmpdir(), `recondo-alias-${randomUUID()}.txt`);
  writeFileSync(file, `alias test ${randomUUID()}`);

  try {
    await page.goto('/upload');
    await page.getByLabel('Relationship', { exact: true }).fill('rel');
    await page.getByLabel('Tag', { exact: true }).fill('aliasa');
    await page.getByRole('button', { name: 'Add pair' }).click();
    await page.locator('#file-input').setInputFiles(file);
    await expect(page.locator('#upload-status')).toContainText('Upload complete');

    await page.goto('/tags/alias');
    await page.getByLabel('Tag A', { exact: true }).fill('aliasa');
    await page.getByLabel('Tag B', { exact: true }).fill('aliasb');
    await page.getByRole('button', { name: 'Alias tags' }).click();
    await expect(page).toHaveURL(/\/tags\/alias/);

    await page.goto('/explore');
    await page.getByRole('searchbox').fill('rel');
    await page.getByRole('button', { name: 'rel' }).click();
    await expect(page.getByRole('link', { name: 'aliasb' })).toBeVisible();
  } finally {
    unlinkSync(file);
  }
});
