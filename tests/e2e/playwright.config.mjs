import { execFileSync } from 'node:child_process';
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '.',
  testMatch: '*.spec.mjs',
  outputDir: 'test-results',
  snapshotPathTemplate: '{testDir}/snapshots/{testFilePath}/{arg}{ext}',
  workers: 1,
  retries: 0,
  forbidOnly: Boolean(process.env.CI),
  timeout: 60_000,
  expect: {
    timeout: 10_000,
    toHaveScreenshot: { animations: 'disabled', maxDiffPixels: 0, threshold: 0 },
  },
  reporter: [
    ['list'],
    ['html', { outputFolder: 'playwright-report', open: 'never' }],
    ['json', { outputFile: 'test-results/report.json' }],
  ],
  metadata: {
    revision: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
    clean: execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).trim() === '',
    environment: 'mcr.microsoft.com/playwright:v1.63.0-noble (linux/amd64)',
  },
  use: {
    browserName: 'chromium',
    viewport: { width: 1280, height: 1000 },
    deviceScaleFactor: 1,
    locale: 'en-US',
    timezoneId: 'UTC',
    colorScheme: 'light',
    reducedMotion: 'reduce',
    serviceWorkers: 'block',
    video: { mode: 'on', size: { width: 1280, height: 1000 } },
    trace: 'retain-on-failure',
  },
});
