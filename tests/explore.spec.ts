import { expect, test, type Page } from './fixtures/test';
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

test('gear icon opens relationship management page and merges source relationship into destination', async ({
  page,
}) => {
  const uniqueSuffix = randomUUID().slice(0, 8);
  const sourceRel = `src rel ${uniqueSuffix}`;
  const destRel = `dest rel ${uniqueSuffix}`;
  const sourceTag = `src tag ${uniqueSuffix}`;
  const destTag = `dest tag ${uniqueSuffix}`;

  const fileA = join(tmpdir(), `recondo-merge-a-${randomUUID()}.txt`);
  const fileB = join(tmpdir(), `recondo-merge-b-${randomUUID()}.txt`);
  writeFileSync(fileA, `Merge source content ${randomUUID()}`);
  writeFileSync(fileB, `Merge dest content ${randomUUID()}`);

  try {
    await page.goto('/upload');
    await page.getByLabel('Relationship', { exact: true }).fill(sourceRel);
    await page.getByLabel('Tag', { exact: true }).fill(sourceTag);
    await page.getByRole('button', { name: 'Add pair' }).click();
    await page.locator('#file-input').setInputFiles(fileA);
    await expect(page.locator('#upload-status')).toContainText('Upload complete');

    await page.goto('/upload');
    await page.getByLabel('Relationship', { exact: true }).fill(destRel);
    await page.getByLabel('Tag', { exact: true }).fill(destTag);
    await page.getByRole('button', { name: 'Add pair' }).click();
    await page.locator('#file-input').setInputFiles(fileB);
    await expect(page.locator('#upload-status')).toContainText('Upload complete');
  } finally {
    unlinkSync(fileA);
    unlinkSync(fileB);
  }

  await page.goto('/explore');
  const search = page.getByRole('searchbox');
  await search.fill(sourceRel);

  const manageLink = page.getByRole('link', { name: `Manage ${sourceRel}` });
  await expect(manageLink).toBeVisible();
  await manageLink.click();

  await expect(page).toHaveURL(/\/relationship\/manage\?name=/);
  await expect(page.getByRole('heading', { level: 1, name: sourceRel })).toBeVisible();
  await expect(page.locator('.relationship-tag-card', { hasText: sourceTag })).toBeVisible();

  const mergeSearch = page.getByRole('searchbox', { name: 'SEARCH DESTINATION RELATIONSHIP' });
  await mergeSearch.fill(destRel);

  const mergeButton = page.getByRole('button', { name: `Merge into ${destRel}` });
  await expect(mergeButton).toBeVisible();
  await mergeButton.click();

  await expect(page.getByRole('heading', { level: 1, name: destRel })).toBeVisible();
  await expect(page.locator('.details-tag-notice')).toContainText(
    `Merged “${sourceRel}” into “${destRel}”.`,
  );
  await expect(page.locator('.relationship-tag-card', { hasText: destTag })).toBeVisible();
  await expect(page.locator('.relationship-tag-card', { hasText: sourceTag })).toBeVisible();

  // Verify the source relationship was deleted on the explore page
  await page.goto('/explore');
  await page.getByRole('searchbox').fill(sourceRel);
  await expect(page.locator('#relationship-suggestions')).toContainText(
    `No relationships match “${sourceRel}”.`,
  );
});

test('rename button is enabled only when search returns 0 results and renames the relationship', async ({
  page,
}) => {
  const uniqueSuffix = randomUUID().slice(0, 8);
  const oldRel = `old rel ${uniqueSuffix}`;
  const existingRel = `existing rel ${uniqueSuffix}`;
  const newRel = `renamed rel ${uniqueSuffix}`;
  const tag = `rename tag ${uniqueSuffix}`;

  const fileA = join(tmpdir(), `recondo-rename-a-${randomUUID()}.txt`);
  const fileB = join(tmpdir(), `recondo-rename-b-${randomUUID()}.txt`);
  writeFileSync(fileA, `Rename source content ${randomUUID()}`);
  writeFileSync(fileB, `Existing rel content ${randomUUID()}`);

  try {
    await page.goto('/upload');
    await page.getByLabel('Relationship', { exact: true }).fill(oldRel);
    await page.getByLabel('Tag', { exact: true }).fill(tag);
    await page.getByRole('button', { name: 'Add pair' }).click();
    await page.locator('#file-input').setInputFiles(fileA);
    await expect(page.locator('#upload-status')).toContainText('Upload complete');

    await page.goto('/upload');
    await page.getByLabel('Relationship', { exact: true }).fill(existingRel);
    await page.getByLabel('Tag', { exact: true }).fill(tag);
    await page.getByRole('button', { name: 'Add pair' }).click();
    await page.locator('#file-input').setInputFiles(fileB);
    await expect(page.locator('#upload-status')).toContainText('Upload complete');
  } finally {
    unlinkSync(fileA);
    unlinkSync(fileB);
  }

  await page.goto(`/relationship/manage?name=${encodeURIComponent(oldRel)}`);
  await expect(page.getByRole('heading', { level: 1, name: oldRel })).toBeVisible();

  const renameButton = page.getByRole('button', { name: 'Rename' });
  await expect(renameButton).toBeDisabled();

  const mergeSearch = page.getByRole('searchbox', { name: 'SEARCH DESTINATION RELATIONSHIP' });
  await mergeSearch.fill(existingRel);
  await expect(page.getByRole('button', { name: `Merge into ${existingRel}` })).toBeVisible();
  await expect(renameButton).toBeDisabled();

  await mergeSearch.fill(newRel);
  await expect(page.locator('#merge-relationship-suggestions')).toContainText(
    `No other relationships match “${newRel}”.`,
  );
  await expect(renameButton).toBeEnabled();
  await renameButton.click();

  await expect(page.getByRole('heading', { level: 1, name: newRel })).toBeVisible();
  await expect(page.locator('.relationship-tag-card', { hasText: tag })).toBeVisible();

  await page.goto('/explore');
  await page.getByRole('searchbox').fill(oldRel);
  await expect(page.locator('#relationship-suggestions')).toContainText(
    `No relationships match “${oldRel}”.`,
  );
});
