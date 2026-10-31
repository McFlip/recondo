import { expect, test, type Page } from '@playwright/test';
import { randomUUID } from 'node:crypto';
import { unlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

async function seedMedia(page: Page) {
  const image = join(process.cwd(), 'ui/web/tests/fixtures/data_trap.png');
  const image2 = join(process.cwd(), 'ui/web/tests/fixtures/exploits_of_a_mom.png');
  const document = join(tmpdir(), `recondo-media-${randomUUID()}.txt`);
  writeFileSync(document, `media page test ${randomUUID()}`);

  try {
    await page.goto('/upload');
    await page.locator('#file-input').setInputFiles([image, image2, document]);
    await expect(page.locator('#upload-status')).toContainText('Upload complete');
  } finally {
    unlinkSync(document);
  }
}

async function seedAdditionalTag(page: Page) {
  const file = join(tmpdir(), `recondo-media-tag-${randomUUID()}.txt`);
  writeFileSync(file, `media page tag test ${randomUUID()}`);

  try {
    await page.goto('/upload');
    await page.getByLabel('Relationship', { exact: true }).fill('subject');
    await page.getByLabel('Tag', { exact: true }).fill('field note');
    await page.getByRole('button', { name: 'Add pair' }).click();
    await page.locator('#file-input').setInputFiles(file);
    await expect(page.locator('#upload-status')).toContainText('Upload complete');
  } finally {
    unlinkSync(file);
  }
}

test('Explore tags open media results with removable selection and media viewers', async ({
  page,
}) => {
  await seedMedia(page);
  await page.goto('/explore');
  await page.getByRole('searchbox').fill('to');
  await page.getByRole('button', { name: 'to do' }).click();
  await page.getByRole('link', { name: 'needs tags' }).click();

  await expect(page).toHaveURL(/\/media\?tag=[a-f0-9]+&limit=10$/);
  await expect(page.locator('.selected-tag-chip')).toContainText('needs tags');
  await expect(page.getByLabel('DISPLAY LIMIT')).toHaveValue('10');
  await expect(page.getByRole('button', { name: 'AND', exact: true })).toBeEnabled();
  await expect(page.getByRole('button', { name: 'OR', exact: true })).toBeEnabled();
  await expect(
    page.getByText('OR then AND is not functional yet.', { exact: true }),
  ).toBeVisible();

  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page.locator('.media-workspace').evaluate(
      (element) => getComputedStyle(element).gridTemplateColumns.split(' ').length,
    ),
  ).toBe(1);
  await page.getByLabel('DISPLAY LIMIT').selectOption('400');
  await expect(page.locator('[data-testid="media-card"]').first()).toBeVisible();
  await expect
    .poll(() =>
      page.locator('.media-preview img').evaluateAll(
        (images) =>
          images.find((image) => (image as HTMLImageElement).naturalWidth > 0)?.getAttribute('src'),
      ),
    )
    .toBeTruthy();
  const allImages = await page.locator('.media-preview img').evaluateAll(
    (images) =>
      images
        .filter((image) => (image as HTMLImageElement).naturalWidth > 0)
        .map((image) => image.getAttribute('src')),
  );
  const imageSrc = allImages[0];
  const image = page.locator(`.media-preview img[src="${imageSrc}"]`);
  expect(
    await image.evaluate((element: HTMLImageElement) => {
      return element.naturalWidth > 0 &&
        element.getBoundingClientRect().width <= element.parentElement!.getBoundingClientRect().width;
    }),
  ).toBe(true);
  await expect(page.locator('.media-download').first()).toHaveAttribute('download', /.+/);

  await image.click();
  await expect(page.locator('#media-lightbox')).toBeVisible();
  await expect(page.locator('#media-lightbox-image')).toBeVisible();

  // Test lightbox navigation arrows and hotkeys
  const prevBtn = page.locator('#media-lightbox-prev');
  const nextBtn = page.locator('#media-lightbox-next');
  await expect(prevBtn).toBeVisible();
  await expect(nextBtn).toBeVisible();

  const initialSrc = await page.locator('#media-lightbox-image').getAttribute('src');
  if (imageSrc === allImages[0]) {
    await expect(prevBtn).toBeDisabled();
    await expect(nextBtn).toBeEnabled();

    // Navigate next via click
    await nextBtn.click();
    await expect(page.locator('#media-lightbox-image')).not.toHaveAttribute('src', initialSrc!);

    // Navigate prev via 'h' hotkey
    await page.keyboard.press('h');
    await expect(page.locator('#media-lightbox-image')).toHaveAttribute('src', initialSrc!);

    // Navigate next via 'l' hotkey
    await page.keyboard.press('l');
    await expect(page.locator('#media-lightbox-image')).not.toHaveAttribute('src', initialSrc!);

    // Navigate prev via ArrowLeft
    await page.keyboard.press('ArrowLeft');
    await expect(page.locator('#media-lightbox-image')).toHaveAttribute('src', initialSrc!);

    // Navigate next via ArrowRight
    await page.keyboard.press('ArrowRight');
    await expect(page.locator('#media-lightbox-image')).not.toHaveAttribute('src', initialSrc!);
  }

  await page.getByRole('button', { name: 'Close image' }).click();
  await expect(page.locator('#media-lightbox')).not.toBeVisible();

  await page.getByRole('link', { name: 'Remove needs tags' }).click();
  await expect(page).toHaveURL('/explore');
  await expect(page.getByRole('heading', { name: 'Explore your collection.' })).toBeVisible();
});

test('OR mode searches relationships and filters selected tags', async ({
  page,
}) => {
  await seedMedia(page);
  await page.goto('/explore');
  await page.getByRole('searchbox').fill('to');
  await page.getByRole('button', { name: 'to do' }).click();
  await page.getByRole('link', { name: 'needs tags' }).click();

  await page.getByRole('button', { name: 'OR', exact: true }).click();
  await expect(page).toHaveURL(/\/media\?tag=[a-f0-9]+&limit=10&operation=or$/);
  await expect(page.getByRole('heading', { name: 'Add tags' })).toBeVisible();

  await page.getByRole('searchbox', { name: 'SEARCH RELATIONSHIPS' }).fill('to');
  await page.getByRole('button', { name: 'to do' }).click();
  await expect(page.locator('#media-relationship-details')).toContainText(
    'No unselected tags are available in this relationship.',
  );
  await expect(page.getByTestId('or-picker-tag')).toHaveCount(0);

  await page.getByRole('button', { name: 'AND', exact: true }).click();
  await expect(page).toHaveURL(/\/media\?tag=[a-f0-9]+&limit=10$/);
  await expect(page.getByRole('heading', { name: 'Related tags' })).toBeVisible();
});

test('AND then OR supports editable groups and preserves their URL state', async ({
  page,
}) => {
  await seedMedia(page);
  await seedAdditionalTag(page);
  await page.goto('/explore');
  await page.getByRole('searchbox').fill('to');
  await page.getByRole('button', { name: 'to do' }).click();
  await page.getByRole('link', { name: 'needs tags' }).click();

  await page.getByRole('button', { name: 'AND then OR', exact: true }).click();
  const groupedUrl = new URL(page.url());
  expect(groupedUrl.searchParams.get('operation')).toBe('and_then_or');
  expect(groupedUrl.searchParams.getAll('group')).toHaveLength(2);
  expect(groupedUrl.searchParams.getAll('target')).toEqual(['0']);
  await expect(page.locator('.media-tag-group')).toHaveCount(2);
  await expect(
    page.getByRole('checkbox', { name: 'Add new tags to this group' }).nth(0),
  ).toBeChecked();
  await expect(
    page.getByRole('checkbox', { name: 'Add new tags to this group' }).nth(1),
  ).not.toBeChecked();

  const operationBox = await page.locator('.operation-control').boundingBox();
  const selectedTagsBox = await page.locator('.selected-tags-section').boundingBox();
  expect(selectedTagsBox!.y).toBeGreaterThan(operationBox!.y);

  await page
    .getByRole('checkbox', { name: 'Add new tags to this group' })
    .nth(0)
    .uncheck();
  await page
    .getByRole('checkbox', { name: 'Add new tags to this group' })
    .nth(1)
    .check();
  await page.getByRole('searchbox', { name: 'SEARCH RELATIONSHIPS' }).fill('sub');
  await page.getByRole('button', { name: 'subject' }).click();
  await page.getByRole('link', { name: 'field note' }).click();

  const taggedGroupsUrl = new URL(page.url());
  const taggedGroups = taggedGroupsUrl.searchParams.getAll('group');
  const fieldNoteId = await page.locator('.media-tag-group .selected-tag-chip')
    .filter({ hasText: 'field note' })
    .first()
    .getAttribute('data-tag-id');
  expect(taggedGroups).toHaveLength(2);
  expect(taggedGroups[0]).not.toContain(fieldNoteId);
  expect(taggedGroups[1]).toContain(fieldNoteId);
  await expect(page.locator('.media-tag-group .selected-tag-chip')).toHaveCount(2);

  await page.getByRole('button', { name: 'OR', exact: true }).click();
  const convertedOrUrl = new URL(page.url());
  expect(convertedOrUrl.searchParams.get('operation')).toBe('or');
  expect(convertedOrUrl.searchParams.getAll('tag')).toContain(fieldNoteId);
  expect(convertedOrUrl.searchParams.getAll('tag')).toHaveLength(2);

  await page.getByRole('button', { name: 'AND then OR', exact: true }).click();
  await expect(page.locator('.media-tag-group')).toHaveCount(2);

  await page.getByRole('button', { name: 'Add group' }).click();
  await expect(page.locator('.media-tag-group')).toHaveCount(3);
  await expect(page.getByText('OR All Of', { exact: true })).toHaveCount(2);

  await page.getByRole('link', { name: 'Delete group 3' }).click();
  await page.getByRole('link', { name: 'Delete group 2' }).click();
  await expect(page).toHaveURL(/\/media\?tag=.*&limit=10$/);
  await expect(page.getByRole('button', { name: 'AND', exact: true })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await expect(page.locator('.media-tag-group')).toHaveCount(0);

  await page.getByRole('button', { name: 'AND then OR', exact: true }).click();
  await page.getByRole('button', { name: 'Add group' }).click();
  await page.getByRole('button', { name: 'OR', exact: true }).click();
  const orUrl = new URL(page.url());
  expect(orUrl.searchParams.get('operation')).toBe('or');
  expect(orUrl.searchParams.getAll('group')).toHaveLength(0);
  expect(orUrl.searchParams.getAll('tag')).toHaveLength(2);
});
