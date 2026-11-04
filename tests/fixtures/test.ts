import { test as base, expect } from '@playwright/test';
import type { Page } from '@playwright/test';
import { serverFixture, type ServerInfo } from './server';

export const test = base.extend<{ server: ServerInfo; page: Page }>({
  ...serverFixture,
  page: async ({ page, server }, use) => {
    const originalGoto = page.goto.bind(page);
    page.goto = (url: string | URL, options?: Parameters<Page['goto']>[1]) => {
      if (typeof url === 'string' && url.startsWith('/')) {
        return originalGoto(server.url + url, options);
      }
      return originalGoto(url, options);
    };
    await use(page);
  },
});

export { expect };
