import { expect, test, type Page } from '@playwright/test';
import { randomBytes } from 'node:crypto';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

/** Minimal valid 1x1 PNG (so `infer` reports `image/png`). */
const PNG_1X1 = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==',
  'base64',
);

/**
 * Every file gets unique content: the test server persists its database for
 * the whole run, so repeated content would come back as DUPLICATE instead of
 * PROCESSED and break assertions in unrelated tests.
 */
const scratch = mkdtempSync(join(tmpdir(), 'recondo-pw-files-'));
let seq = 0;

function scratchFile(name: string, content: Buffer | string): string {
  const unique = `${Date.now()}-${seq++}-${randomBytes(4).toString('hex')}`;
  const path = join(scratch, `${unique}-${name}`);
  writeFileSync(path, content);
  return path;
}

/** Console noise (errors/warnings/page exceptions) collected after setup. */
function collectConsoleIssues(page: Page): string[] {
  const issues: string[] = [];
  page.on('console', (msg) => {
    if (msg.type() === 'error' || msg.type() === 'warning') {
      issues.push(`${msg.type()}: ${msg.text()}`);
    }
  });
  page.on('pageerror', (err) => issues.push(`pageerror: ${err.message}`));
  return issues;
}

test.beforeEach(async ({ page }) => {
  // The app ships no favicon; swallow the browser's automatic request so the
  // console-cleanliness assertions below only see real application problems.
  await page.route('**/favicon.ico', (route) => route.fulfill({ status: 204, body: '' }));
  await page.goto('/upload');
});

test('renders the dropzone with Datastar v1 drag-drop wiring', async ({ page }) => {
  const dropzone = page.locator('#dropzone');

  await expect(dropzone).toBeVisible();
  await expect(page.locator('#feed-placeholder')).toBeVisible();

  // Datastar v1 splits attribute modifiers on `__`, not on `:`. With a second
  // `:` the event name becomes "dragover:prevent" — a non-existent DOM event —
  // the handler never attaches, and dropping silently does nothing. (Found the
  // hard way: markup looked right but drag-and-drop was dead in the browser.)
  await expect(dropzone).toHaveAttribute('data-on:dragover__prevent', '$dragging = true');
  await expect(dropzone).toHaveAttribute('data-on:drop__prevent', /@post\('\/upload'/);
  await expect(dropzone).not.toHaveAttribute('data-on:dragover:prevent', /.*/);
  await expect(dropzone).not.toHaveAttribute('data-on:drop:prevent', /.*/);
});

test('drag-and-drop uploads multiple files as processed cards', async ({ page }) => {
  const issues = collectConsoleIssues(page);
  const files = [
    scratchFile('note.txt', `drop test ${randomBytes(8).toString('hex')}`),
    scratchFile('image.png', PNG_1X1),
  ];

  // Playwright refuses the drop unless dragover called preventDefault(), so
  // this line fails if the `__prevent` modifiers regress.
  await page.locator('#dropzone').drop({ files });

  await expect(page.locator('#feed-placeholder')).toHaveCount(0);
  await expect(page.locator('.processed-card')).toHaveCount(2);
  await expect(page.locator('.status-tag', { hasText: 'PROCESSED' })).toHaveCount(2);
  await expect(page.getByRole('heading', { name: 'note.txt' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'image.png' })).toBeVisible();
  await expect(page.getByText('image/png', { exact: true })).toBeVisible();

  const status = page.locator('#upload-status');
  await expect(status).toContainText('Upload complete');
  await expect(status).toContainText('100%');
  await expect(status).not.toHaveClass(/is-errored/);

  // No Datastar warnings (e.g. PatchElementsNoTargetsFound from a stale
  // #feed-placeholder patch) and no page exceptions.
  expect(issues).toEqual([]);
});

test('file-input selection uploads and clears the input afterwards', async ({ page }) => {
  const file = scratchFile('picked.txt', `picker test ${randomBytes(8).toString('hex')}`);

  // setInputFiles assigns the FileList and fires `change` — no OS dialog.
  await page.locator('#file-input').setInputFiles(file);

  await expect(page.locator('.status-tag', { hasText: 'PROCESSED' })).toHaveCount(1);
  await expect(page.getByRole('heading', { name: 'picked.txt' })).toBeVisible();
  await expect(page.locator('#upload-status')).toContainText('Upload complete');
  await expect(page.locator('#file-input')).toHaveValue('');
});

test('duplicate content is marked as a duplicate, not an error', async ({ page }) => {
  const file = scratchFile('dup.txt', `duplicate test ${randomBytes(8).toString('hex')}`);

  await page.locator('#dropzone').drop({ files: [file] });
  await expect(page.locator('.status-tag', { hasText: 'PROCESSED' })).toHaveCount(1);

  await page.locator('#dropzone').drop({ files: [file] });
  await expect(page.locator('.status-tag', { hasText: 'DUPLICATE' })).toHaveCount(1);

  const status = page.locator('#upload-status');
  await expect(status).toContainText('Upload complete');
  await expect(status).not.toHaveClass(/is-errored/);
});

test('shows an indeterminate bar while uploading, then honest progress', async ({ page }) => {
  // ~48 MB of random data: large enough that the upload phase outlives a
  // mutation-observer tick, so the indeterminate state is observable. Random
  // bytes also guarantee unique content (no accidental duplicates).
  const files = [1, 2, 3, 4].map((i) =>
    scratchFile(`big${i}.bin`, randomBytes(12 * 1024 * 1024)),
  );

  // Record every visual state of the status area so we can assert on the
  // intermediate states that a point-in-time assertion would miss.
  await page.evaluate(() => {
    const win = window as unknown as {
      __progressLog: Array<{ text: string; barClass: string }>;
    };
    win.__progressLog = [];
    const status = document.getElementById('upload-status')!;
    const bar = document.querySelector('.progress-bar')!;
    const record = () =>
      win.__progressLog.push({
        text: (status.textContent || '').replace(/\s+/g, ' ').trim(),
        barClass: bar.className,
      });
    new MutationObserver(record).observe(status, {
      subtree: true,
      childList: true,
      attributes: true,
      characterData: true,
    });
    record();
  });

  await page.locator('#dropzone').drop({ files });

  await expect(page.locator('#upload-status')).toContainText('Upload complete', {
    timeout: 60_000,
  });

  const log = await page.evaluate(
    () => (window as unknown as { __progressLog: Array<{ text: string; barClass: string }> }).__progressLog,
  );

  // While bytes are still streaming, the bar must animate (indeterminate)
  // instead of showing a fake percentage.
  expect(
    log.some((s) => s.text.includes('Uploading files...') && s.barClass.includes('indeterminate')),
    `expected an indeterminate upload phase, got: ${JSON.stringify(log)}`,
  ).toBe(true);

  // Per-file progress states may be skipped by observer coalescing, but the
  // final state must be complete and no longer indeterminate.
  expect(log.at(-1)?.text).toContain('Upload complete');
  expect(log.at(-1)?.text).toContain('100%');
  expect(log.at(-1)?.barClass).not.toContain('indeterminate');
});

test('a failed upload shows an error state and recovers on the next one', async ({ page }) => {
  const status = page.locator('#upload-status');

  // Simulate a network/server failure on the upload request itself.
  await page.route('**/upload', (route) => route.fulfill({ status: 500, body: 'boom' }));
  await page.locator('#dropzone').drop({ files: [scratchFile('fail.txt', randomBytes(16))] });

  // The failure must be visible — never masked as success.
  await expect(status).toContainText('Upload failed');
  await expect(status).toHaveClass(/is-errored/);
  await expect(page.locator('.processed-card')).toHaveCount(0);

  // Once the server is healthy again, the error state must reset.
  await page.unroute('**/upload');
  await page.locator('#dropzone').drop({ files: [scratchFile('ok.txt', randomBytes(16))] });

  await expect(status).toContainText('Upload complete');
  await expect(status).not.toHaveClass(/is-errored/);
  await expect(page.locator('.status-tag', { hasText: 'PROCESSED' })).toHaveCount(1);
});
