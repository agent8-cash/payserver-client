import { defineConfig } from '@playwright/test';
import { resolve } from 'node:path';

/**
 * Layout regression tests only.
 *
 * These came across with the frontend when it left ethpayserver, and they
 * belong here for the same reason the stylesheet does: a CSS specificity
 * regression should fail the repository that owns the CSS, not a payserver's
 * pipeline. They need no server, no database and no auth — each test injects
 * `styles.css` into a blank page and asserts computed style — so this config
 * deliberately has no `webServer` and no fixtures.
 */

// Local runs only. CI's /tmp is disk-backed and the runner is thrown away
// after the job; this box's own /tmp is RAM-backed, which is where
// Playwright puts the chromium profile directory by default. Setting
// TMPDIR here, at config load, redirects it for every entry point that
// loads this config — `npx playwright test`, `npm test`, `--ui` alike —
// rather than only ones that go through a wrapper script, which a bare
// `npx playwright test` bypasses entirely.
//
// This assignment only ever points at the same deterministic path, so it's
// safe to run once per process with no coordination. The directory itself is
// created (and wiped of anything a killed previous run left behind) in
// globalSetup instead of here, because this module is re-imported by every
// `fullyParallel` worker independently — an `rmSync`/`mkdirSync` pair run
// from here would race a sibling worker that already has a browser profile
// open under the same path. globalSetup runs exactly once, before any worker
// starts.
if (!process.env.CI) {
  process.env.TMPDIR = resolve(process.cwd(), '.tmp');
}

export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: 0,
  globalSetup: process.env.CI ? undefined : require.resolve('./playwright.global-setup.ts'),
  reporter: process.env.CI ? [['github'], ['list']] : [['list']],
  use: { browserName: 'chromium' },
});
