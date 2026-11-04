import { spawn, type ChildProcess } from 'node:child_process';
import { mkdtempSync, rmSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createServer } from 'node:net';
import type { Fixtures } from '@playwright/test';

export interface ServerInfo {
  url: string;
  dir: string;
}

function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const srv = createServer();
    srv.listen(0, '127.0.0.1', () => {
      const addr = srv.address();
      if (addr && typeof addr === 'object') {
        const port = addr.port;
        srv.close(() => resolve(port));
      } else {
        reject(new Error('no address'));
      }
    });
    srv.on('error', reject);
  });
}

async function waitForReady(url: string, timeoutMs = 30_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(url);
      if (res.ok) return;
    } catch {
      // not ready yet
    }
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error(`server not ready: ${url}`);
}

export const serverFixture: Fixtures<{ server: ServerInfo }, {}> = {
  server: async ({}, use) => {
    const binary = readFileSync(join(process.cwd(), '.pw-binary-path'), 'utf8').trim();
    const dir = mkdtempSync(join(tmpdir(), 'recondo-pw-'));
    const port = await freePort();
    const url = `http://127.0.0.1:${port}`;

    const proc: ChildProcess = spawn(binary, [], {
      cwd: dir,
      env: {
        ...process.env,
        RECONDO_DB_PATH: join(dir, 'graph.redb'),
        RECONDO_ADDR: `127.0.0.1:${port}`,
      },
      stdio: ['ignore', 'pipe', 'pipe'],
    });

    let stderr = '';
    proc.stderr?.on('data', (d) => {
      stderr += d.toString();
    });

    try {
      await waitForReady(`${url}/upload`);
      await use({ url, dir });
    } finally {
      proc.kill('SIGTERM');
      await new Promise<void>((resolve) => {
        const t = setTimeout(() => {
          try {
            proc.kill('SIGKILL');
          } catch {
            // ignore
          }
          resolve();
        }, 5000);
        proc.on('exit', () => {
          clearTimeout(t);
          resolve();
        });
      });
      rmSync(dir, { recursive: true, force: true });
      if (stderr && process.env.PW_DEBUG) console.error(stderr);
    }
  },
};
