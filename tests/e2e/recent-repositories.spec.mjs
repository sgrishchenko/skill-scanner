import { test, expect } from '@playwright/test';
import { LocalApp, mockScans } from './support.mjs';

test('recent repositories persist, update, and can be removed without losing results', async ({ page, context }, testInfo) => {
  const app = new LocalApp(testInfo);
  const mock = await mockScans(context, app);
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  const repositories = page.locator('.recent-repository');
  const input = page.getByRole('textbox', { name: 'GitHub repository' });
  const scan = page.getByRole('button', { name: 'Scan repository', exact: true });

  async function checkpoint(name) {
    await page.locator('.scan-panel').evaluate((panel) => window.scrollTo(0, panel.offsetTop - 24));
    await page.mouse.move(0, 0);
    await expect(page).toHaveScreenshot(`${name}.png`, { fullPage: true });
    testInfo.annotations.push({ type: 'checkpoint', description: name });
    // Assertions above determine readiness. This pause only makes the same
    // passing test's video readable; it is never used to synchronize the UI.
    await page.waitForTimeout(1_200);
  }

  async function scanning(expectedRequests) {
    await expect.poll(() => mock.requests.length).toBe(expectedRequests);
    await expect(input).toBeDisabled();
    await expect(page.getByRole('button', { name: 'Scanning…', exact: true })).toBeDisabled();
    await expect(page.locator('#scan-results')).toBeHidden();
    for (const button of await page.locator('#recent-list button').all()) {
      await expect(button).toBeDisabled();
    }
  }

  async function completed(repository, count) {
    await expect(scan).toBeEnabled();
    await expect(page.locator('#results-status')).toHaveText('Scan complete');
    await expect(page.locator('#result-repository')).toHaveText(repository);
    await expect(page.locator('#skill-count')).toHaveText(String(count));
    await expect(page.locator('#skill-list .skill-card')).toHaveCount(count);
  }

  try {
    await app.start();
    await page.goto(app.url);
    await expect(page.locator('#recent-status')).toHaveText('No recently scanned repositories yet.');
    await expect(repositories).toHaveCount(0);
    await expect(page.locator('#initial-state')).toBeVisible();
    await expect(page.locator('#scan-results')).toBeHidden();
    await checkpoint('01-empty-history');

    await input.fill('example/review-tools');
    await scan.click();
    await scanning(1);
    await checkpoint('02-scanning');
    await mock.complete();
    await completed('example/review-tools', 2);
    await expect(repositories).toHaveText(['example/review-tools']);
    await expect(page.locator('#recent-list time')).toHaveAttribute('datetime', '2026-09-01T12:00:00.000Z');
    await checkpoint('03-first-scan');

    await input.fill('example/release-tools');
    await scan.click();
    await scanning(2);
    await mock.complete();
    await completed('example/release-tools', 1);
    await expect(repositories).toHaveText(['example/release-tools', 'example/review-tools']);
    await expect(page.locator('.recent-row .recent-hint')).toHaveText([
      '1 skill · Last scanned 9/1/2026, 12:05:00 PM',
      '2 skills · Last scanned 9/1/2026, 12:00:00 PM',
    ]);
    await checkpoint('04-newest-first');

    await app.stop();
    await app.start();
    await page.goto(app.url);
    await expect(repositories).toHaveText(['example/release-tools', 'example/review-tools']);
    await expect(page.locator('#scan-results')).toBeHidden();
    await expect(page.locator('#initial-state')).toBeVisible();
    const shared = app.recent();
    expect(shared).toMatch(/example\/release-tools.*1 skill/);
    expect(shared).toMatch(/example\/review-tools.*2 skills/);
    expect(shared.indexOf('example/release-tools')).toBeLessThan(shared.indexOf('example/review-tools'));
    await checkpoint('05-restart-persistence');

    // Keyboard selection must only populate and focus the input, without scanning.
    await page.getByRole('button', { name: 'Use example/review-tools for another scan', exact: true }).focus();
    await page.keyboard.press('Enter');
    await expect(input).toHaveValue('example/review-tools');
    await expect(input).toBeFocused();
    await expect(page.locator('#scan-results')).toBeHidden();
    expect(mock.requests).toHaveLength(2);
    await checkpoint('06-select-saved-repository');

    await scan.click();
    await scanning(3);
    await mock.complete();
    await completed('example/review-tools', 3);
    await expect(repositories).toHaveText(['example/review-tools', 'example/release-tools']);
    await expect(page.locator('.recent-row .recent-hint').first()).toHaveText('3 skills · Last scanned 9/1/2026, 12:10:00 PM');
    await expect(page.locator('#recent-list time').first()).toHaveAttribute('datetime', '2026-09-01T12:10:00.000Z');
    await checkpoint('07-repeat-scan-updates-entry');

    await page.getByRole('button', { name: 'Remove example/review-tools from recent repositories', exact: true }).click();
    await expect(repositories).toHaveText(['example/release-tools']);
    await expect(page.locator('#recent-status')).toHaveText('Removed example/review-tools from recent repositories.');
    await expect(page.getByRole('button', { name: 'Refresh list', exact: true })).toBeFocused();
    await completed('example/review-tools', 3);
    await checkpoint('08-removal-keeps-current-results');

    await app.stop();
    await app.start();
    await page.goto(app.url);
    await expect(repositories).toHaveText(['example/release-tools']);
    await expect(page.locator('#scan-results')).toBeHidden();
    await expect(page.locator('#initial-state')).toBeVisible();
    const remaining = app.recent();
    expect(remaining).toMatch(/example\/release-tools.*1 skill/);
    expect(remaining).not.toContain('example/review-tools');
    await checkpoint('09-removal-persists-after-restart');

    mock.assertFinished();
    expect(errors).toEqual([]);
  } finally {
    await app.stop();
  }
});
