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
    page.getByText('AND then OR and OR then AND are not functional yet.', { exact: true }),
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
