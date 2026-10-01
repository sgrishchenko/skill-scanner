import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { test, expect } from '@playwright/test';
import { LocalApp, mockCodexInstalls, mockScans } from './support.mjs';

const commit = '1111111111111111111111111111111111111111';
const reviewInstall = {
  repository: 'example/review-tools', path: 'skills/code-review/SKILL.md', commit,
  name: 'code-review', description: 'Review changes for correctness and maintainability.',
  installed_at: 1788264100000,
};
const releaseInstall = {
  repository: 'example/release-tools', path: 'skills/release-notes/SKILL.md', commit,
  name: 'release-notes', description: 'Summarize changes for a release.',
  installed_at: 1788264400000,
};

test('skills install for Codex, survive restarts, refuse name conflicts, and can be removed', async ({ page, context }, testInfo) => {
  const app = new LocalApp(testInfo);
  const mock = await mockScans(context, app, 'codex-scans.json');
  const installs = await mockCodexInstalls(context, app, [reviewInstall, releaseInstall]);
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  const input = page.getByRole('textbox', { name: 'GitHub repository' });
  const scan = page.getByRole('button', { name: 'Scan repository', exact: true });
  const cards = page.locator('#skill-list .skill-card');
  const codexNames = page.locator('#codex-list .starred-name');
  const codexStatus = page.locator('#codex-status');
  const codexError = page.locator('#codex-error');
  const install = (skillPath) => page.getByRole('button', { name: `Install ${skillPath} for Codex`, exact: true });
  const remove = (skillPath) => page.getByRole('button', { name: `Remove ${skillPath} from Codex`, exact: true });
  const folder = (name) => path.join(app.codexDirectory, name);

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
    await expect(codexStatus).toHaveText('No skills installed for Codex yet.');
    await expect(page.locator('#codex-hint code')).toHaveText(app.codexDirectory);
    await expect(codexNames).toHaveCount(0);
    await scanRepository('example/review-tools', 2);
    await expect(install('skills/code-review/SKILL.md')).toHaveText('Install for Codex');
    await expect(install('skills/test-plan/SKILL.md')).toHaveText('Install for Codex');
    expect(app.codex()).toContain('No Codex skills installed by Skill Scanner');
    await checkpoint('01-ready-to-install', '#codex-title');

    // The pending installation keeps focus on its button and ignores repeats.
    const reviewButton = install('skills/code-review/SKILL.md');
    await reviewButton.click();
    await expect(reviewButton).toHaveText('Installing…');
    await expect(reviewButton).toHaveAttribute('aria-disabled', 'true');
    await expect(reviewButton).toBeFocused();
    await expect(codexStatus).toHaveText('Installing code-review for Codex…');
    await reviewButton.click({ force: true });
    await checkpoint('02-installing', '#codex-title');

    await installs.complete(reviewInstall);
    await expect(remove('skills/code-review/SKILL.md')).toHaveText('Remove from Codex');
    await expect(remove('skills/code-review/SKILL.md')).toBeFocused();
    await expect(remove('skills/code-review/SKILL.md')).toHaveAttribute('aria-disabled', 'false');
    await expect(codexStatus).toHaveText('Installed code-review for Codex.');
    await expect(codexNames).toHaveText(['code-review']);
    await expect(page.locator('#codex-list .recent-hint')).toHaveText(['example/review-tools · skills/code-review/SKILL.md']);
    await expect(page.getByRole('link', { name: 'View the installed skills/code-review/SKILL.md from example/review-tools on GitHub (opens in a new tab)' }))
      .toHaveAttribute('href', `https://github.com/example/review-tools/blob/${commit}/skills/code-review/SKILL.md`);
    expect(installs.requests).toHaveLength(1);
    expect(app.codex()).toContain('code-review\n  Repository: example/review-tools\n  Path: skills/code-review/SKILL.md');
    await checkpoint('03-installed', '#codex-title');

    // Another repository's code-review skill would replace the installed
    // folder, so the actual server refuses it before contacting GitHub.
    await scanRepository('example/release-tools', 2);
    await expect(remove('skills/code-review/SKILL.md')).toHaveCount(0);
    await install('skills/code-review/SKILL.md').click();
    await expect(codexError).toHaveText('The Codex skill folder code-review already holds skills/code-review/SKILL.md from example/review-tools. Remove it first to install this skill.');
    await expect(install('skills/code-review/SKILL.md')).toHaveText('Install for Codex');
    await expect(install('skills/code-review/SKILL.md')).toBeFocused();
    await expect(codexNames).toHaveText(['code-review']);
    expect(readFileSync(path.join(folder('code-review'), 'SKILL.md'), 'utf8')).toContain('Review changes for correctness');
    await checkpoint('04-name-conflict', '#codex-title');

    // Keyboard activation installs a differently named skill.
    await install('skills/release-notes/SKILL.md').focus();
    await page.keyboard.press('Enter');
    await installs.complete(releaseInstall);
    await expect(remove('skills/release-notes/SKILL.md')).toBeFocused();
    await expect(codexError).toBeHidden();
    await expect(codexStatus).toHaveText('Installed release-notes for Codex.');
    await expect(codexNames).toHaveText(['code-review', 'release-notes']);
    expect(installs.requests).toHaveLength(3);
    await checkpoint('05-second-install', '#codex-title');

    await restart();
    await expect(codexNames).toHaveText(['code-review', 'release-notes']);
    await expect(codexStatus).toHaveText('Installed by Skill Scanner, sorted by folder name.');
    expect(app.codex()).toMatch(/^Codex skills installed by Skill Scanner in .*:\n\ncode-review\n.*\n\nrelease-notes\n/s);
    await checkpoint('06-restart-persistence', '#codex-title');

    // Removal from the list deletes the folder and moves focus to the heading.
    await page.getByRole('button', { name: 'Remove code-review from Codex', exact: true }).click();
    await expect(codexNames).toHaveText(['release-notes']);
    await expect(codexStatus).toHaveText('Removed code-review from Codex.');
    await expect(page.locator('#codex-title')).toBeFocused();
    expect(existsSync(folder('code-review'))).toBe(false);
    await checkpoint('07-remove-from-list', '#codex-title');

    await scanRepository('example/release-tools', 2);
    await expect(install('skills/code-review/SKILL.md')).toHaveText('Install for Codex');
    await remove('skills/release-notes/SKILL.md').click();
    await expect(install('skills/release-notes/SKILL.md')).toHaveText('Install for Codex');
    await expect(install('skills/release-notes/SKILL.md')).toBeFocused();
    await expect(codexNames).toHaveCount(0);
    await expect(codexStatus).toHaveText('Removed release-notes from Codex.');
    expect(existsSync(folder('release-notes'))).toBe(false);
    expect(app.codex()).toContain('No Codex skills installed by Skill Scanner');
    await checkpoint('08-remove-from-card', '#codex-title');

    mock.assertFinished();
    installs.assertFinished();
    expect(installs.requests).toHaveLength(3);
    expect(errors).toEqual([]);
  } finally {
    await app.stop();
  }
});
