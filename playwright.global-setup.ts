import { mkdirSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';

// Playwright runs this exactly once, in the main process, before any worker
// starts — unlike playwright.config.ts, which every `fullyParallel` worker
// re-imports independently. Doing the rm/mkdir here, rather than in the
// config body, is what stops concurrent workers from racing to wipe the
// scratch directory out from under each other's live browser profile.
export default function globalSetup() {
  const scratch = resolve(process.cwd(), '.tmp');
  rmSync(scratch, { recursive: true, force: true });
  mkdirSync(scratch, { recursive: true });
}
