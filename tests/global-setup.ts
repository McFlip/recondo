import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { join } from 'node:path';

// Build the web binary once so per-test spawns are fast.
export default function globalSetup() {
  const manifest = join(process.cwd(), 'Cargo.toml');
  execFileSync('cargo', ['build', '-p', 'recondo-web', '--quiet', '--manifest-path', manifest], {
    stdio: 'inherit',
  });
  const binary = join(process.cwd(), 'target', 'debug', 'recondo-web');
  writeFileSync(join(process.cwd(), '.pw-binary-path'), binary);
}
