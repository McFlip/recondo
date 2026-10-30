import { expect, test, type Page } from '@playwright/test';
import { randomUUID } from 'node:crypto';
import { unlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

async function seedRelationship(page: Page) {
  const file = join(tmpdir(), `recondo-explore-${randomUUID()}.txt`);
  writeFileSync(file, `Explore relationship test ${randomUUID()}`);

  try {
    await page.goto('/upload');
    await page.locator('#file-input').setInputFiles(file);
    await expect(page.locator('#upload-status')).toContainText('Upload complete');
  } finally {
    unlinkSync(file);
  }
}

test('live search lets a user click a relationship to see its tags', async ({ page }) => {
  await seedRelationship(page);
  await page.goto('/explore');

  const search = page.getByRole('searchbox');
  await search.fill('to');

  const relationship = page.getByRole('button', { name: 'to do' });
  await expect(relationship).toBeVisible();
  await relationship.click();

  await expect(page.locator('#relationship-details h2')).toHaveText('to do');
  await expect(page.locator('.relationship-tag-card')).toContainText('needs tags');
  await expect(page.locator('.relationship-tag-grid')).toHaveCSS('display', 'grid');
});

test('pressing Enter selects the first relationship matching the prefix', async ({ page }) => {
  await seedRelationship(page);
  await page.goto('/explore');

  const search = page.getByRole('searchbox');
  await search.fill('t');
  await expect(page.getByRole('button', { name: 'to do' })).toBeVisible();
  await search.press('Enter');

  await expect(page.locator('#relationship-details h2')).toHaveText('to do');
  await expect(page.locator('.relationship-tag-card')).toContainText('needs tags');
});
