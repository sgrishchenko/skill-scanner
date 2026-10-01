import { test, expect } from '@playwright/test';
import { LocalApp, mockScans } from './support.mjs';

const commit = '1111111111111111111111111111111111111111';

test('starred skills persist across scans and restarts and can be unstarred', async ({ page, context }, testInfo) => {
  const app = new LocalApp(testInfo);
  const mock = await mockScans(context, app);
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  const input = page.getByRole('textbox', { name: 'GitHub repository' });
  const scan = page.getByRole('button', { name: 'Scan repository', exact: true });
  const starredOnly = page.getByRole('checkbox', { name: 'Starred only' });
  const cards = page.locator('#skill-list .skill-card');
  const starredNames = page.locator('#starred-list .starred-name');
  const starredStatus = page.locator('#starred-status');
  const star = (path) => page.getByRole('button', { name: `Star ${path}`, exact: true });

  async function checkpoint(name, focus) {
    // Scroll the video to the area that changed; screenshots cover the full page.
    await page.locator(focus).evaluate((element) => window.scrollTo(0, element.getBoundingClientRect().top + window.scrollY - 24));
    await page.mouse.move(0, 0);
    await expect(page).toHaveScreenshot(`${name}.png`, { fullPage: true });
    testInfo.annotations.push({ type: 'checkpoint', description: name });
    // Assertions above determine readiness. This pause only makes the same
    // passing test's video readable; it is never used to synchronize the UI.
    await page.waitForTimeout(1_200);
  }

  async function scanRepository(repository, count) {
    const requests = mock.requests.length;
    await input.fill(repository);
    await scan.click();
    await expect.poll(() => mock.requests.length).toBe(requests + 1);
    await expect(page.locator('#scan-results')).toBeHidden();
    await mock.complete();
    await expect(scan).toBeEnabled();
    await expect(page.locator('#results-status')).toHaveText('Scan complete');
    await expect(page.locator('#result-repository')).toHaveText(repository);
    await expect(cards).toHaveCount(count);
    await expect(starredOnly).not.toBeChecked();
  }

  async function expectStars(expected) {
    for (const [path, pressed] of Object.entries(expected)) {
      await expect(star(path)).toHaveAttribute('aria-pressed', String(pressed));
      await expect(star(path)).toHaveText(pressed ? '★Starred' : '☆Star');
    }
  }

  async function restart() {
    await app.stop();
    await app.start();
    await page.goto(app.url);
    await expect(page.locator('#scan-results')).toBeHidden();
    await expect(page.locator('#initial-state')).toBeVisible();
  }

  try {
    await app.start();
    await page.goto(app.url);
    await expect(starredStatus).toHaveText('No starred skills yet.');
    await expect(starredNames).toHaveCount(0);
    await scanRepository('example/review-tools', 2);
    await expectStars({ 'skills/code-review/SKILL.md': false, 'skills/test-plan/SKILL.md': false });
    await checkpoint('01-unstarred-results', '#starred-title');

    await star('skills/test-plan/SKILL.md').click();
    await expectStars({ 'skills/code-review/SKILL.md': false, 'skills/test-plan/SKILL.md': true });
    await expect(star('skills/test-plan/SKILL.md')).toBeFocused();
    await expect(starredStatus).toHaveText('Starred test-plan.');
    await expect(starredNames).toHaveText(['test-plan']);
    await expect(page.locator('#starred-list .recent-hint')).toHaveText(['example/review-tools · skills/test-plan/SKILL.md']);
    await expect(page.getByRole('link', { name: 'View skills/test-plan/SKILL.md in example/review-tools on GitHub (opens in a new tab)' }))
      .toHaveAttribute('href', `https://github.com/example/review-tools/blob/${commit}/skills/test-plan/SKILL.md`);
    expect(app.starred()).toContain('test-plan\n  Repository: example/review-tools\n  Path: skills/test-plan/SKILL.md');
    await checkpoint('02-star-skill', '#starred-title');

    await starredOnly.check();
    await expect(cards).toHaveCount(1);
    await expect(cards.locator('h3')).toHaveText(['test-plan']);
    await expect(page.locator('#result-count')).toHaveText('Showing 1 of 2 skills in 1 of 2 groups (including standalone skills) · largest first');
    await checkpoint('03-starred-only-filter', '#result-controls');

    await scanRepository('example/release-tools', 1);
    // Keyboard activation stars a skill from another repository.
    await star('skills/release-notes/SKILL.md').focus();
    await page.keyboard.press('Enter');
    await expectStars({ 'skills/release-notes/SKILL.md': true });
    await expect(starredNames).toHaveText(['release-notes', 'test-plan']);
    await expect(starredStatus).toHaveText('Starred release-notes.');
    await checkpoint('04-stars-across-repositories', '#starred-title');

    await restart();
    await expect(starredNames).toHaveText(['release-notes', 'test-plan']);
    await expect(starredStatus).toHaveText('Most recently starred first.');
    const shared = app.starred();
    expect(shared).toMatch(/^Starred skills \(newest first\):\n\nrelease-notes\n.*\n\ntest-plan\n/s);
    await checkpoint('05-restart-persistence', '#starred-title');

    // A new scan of the same repository shows the persisted star on the same path.
    await scanRepository('example/review-tools', 3);
    await expectStars({
      'skills/code-review/SKILL.md': false,
      'skills/security-review/SKILL.md': false,
      'skills/test-plan/SKILL.md': true,
    });
    await checkpoint('06-rescan-keeps-stars', '#result-controls');

    await star('skills/test-plan/SKILL.md').click();
    await expectStars({ 'skills/test-plan/SKILL.md': false });
    await expect(starredNames).toHaveText(['release-notes']);
    await expect(starredStatus).toHaveText('Unstarred test-plan.');
    await checkpoint('07-unstar-from-results', '#starred-title');

    await page.getByRole('button', { name: 'Unstar skills/release-notes/SKILL.md in example/release-tools', exact: true }).click();
    await expect(starredNames).toHaveCount(0);
    await expect(starredStatus).toHaveText('Unstarred release-notes.');
    await expect(page.locator('#starred-title')).toBeFocused();
    await expect(cards).toHaveCount(3);
    await expectStars({ 'skills/test-plan/SKILL.md': false });
    await checkpoint('08-unstar-from-list', '#starred-title');

    await restart();
    await expect(starredStatus).toHaveText('No starred skills yet.');
    await expect(starredNames).toHaveCount(0);
    expect(app.starred()).toContain('No starred skills.');
    await checkpoint('09-unstar-persists-after-restart', '#starred-title');

    mock.assertFinished();
    expect(errors).toEqual([]);
  } finally {
    await app.stop();
  }
});
