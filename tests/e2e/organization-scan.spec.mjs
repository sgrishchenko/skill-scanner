import { test, expect } from '@playwright/test';
import { LocalApp, checkpoint as compare, mockOrganizationScans } from './support.mjs';

test('organization scans combine repository results, report failures, and stop on rate limits', async ({ page, context }, testInfo) => {
  const app = new LocalApp(testInfo);
  const mock = await mockOrganizationScans(context, app);
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  const checkpoint = (name) => compare(page, testInfo, name);
  const repositoryMode = page.getByRole('button', { name: 'Repository', exact: true });
  const organizationMode = page.getByRole('button', { name: 'Organization', exact: true });
  const scan = page.getByRole('button', { name: 'Scan organization', exact: true });

  async function scanning(expectedRequests) {
    await expect.poll(() => mock.requests.length).toBe(expectedRequests);
    const input = page.getByRole('textbox', { name: 'GitHub organization or user' });
    await expect(input).toBeDisabled();
    await expect(page.getByRole('button', { name: 'Scanning…', exact: true })).toBeDisabled();
    await expect(repositoryMode).toBeDisabled();
    await expect(organizationMode).toBeDisabled();
    for (const button of await page.locator('#organization-examples button').all()) {
      await expect(button).toBeDisabled();
    }
    await expect(page.locator('#results-status')).toHaveText('Scanning organization…');
    await expect(page.locator('#scan-results')).toBeHidden();
  }

  try {
    await app.start();
    await page.goto(app.url);
    await expect(page.locator('#recent-status')).toHaveText('No recently scanned repositories yet.');
    await expect(repositoryMode).toHaveAttribute('aria-pressed', 'true');

    // Switching modes changes the form without scanning; examples only fill the field.
    await organizationMode.click();
    await expect(organizationMode).toHaveAttribute('aria-pressed', 'true');
    await expect(repositoryMode).toHaveAttribute('aria-pressed', 'false');
    await expect(page.locator('#scan-title')).toHaveText('Start with an organization');
    const input = page.getByRole('textbox', { name: 'GitHub organization or user' });
    await expect(input).toHaveAttribute('placeholder', 'organization or https://github.com/organization');
    await expect(page.locator('#organization-hint')).toBeVisible();
    await expect(page.locator('#repository-examples')).toBeHidden();
    await page.getByRole('button', { name: 'JetBrains', exact: true }).click();
    await expect(input).toHaveValue('JetBrains');
    await expect(input).toBeFocused();
    await input.fill('https://github.com/example');
    await expect(scan).toBeEnabled();
    expect(mock.requests).toHaveLength(0);
    await checkpoint('01-organization-mode');

    await scan.click();
    await scanning(1);
    await checkpoint('02-scanning-organization');

    await mock.complete();
    await expect(scan).toBeEnabled();
    await expect(page.locator('#results-status')).toHaveText('Incomplete · 1 repository failed');
    await expect(page.locator('#result-kind')).toHaveText('ORGANIZATION');
    await expect(page.locator('#result-repository')).toHaveText('example');
    await expect(page.locator('#result-commit')).toHaveText('5 public repositories scanned · 2 with skills · 2 forks skipped');
    await expect(page.locator('#incomplete-panel')).toContainText('1 of 5 repositories could not be scanned');
    await expect(page.locator('#repository-results > li')).toHaveText([
      /^example\/huge-monorepo.*scan this repository individually\.$/,
      /^example\/agent-tools2 skills · Commit 1111111 ↗$/,
      /^example\/review-bot1 skill · Commit 3333333 ↗$/,
    ]);
    await expect(page.getByRole('link', { name: 'Scanned commit 3333333333333333333333333333333333333333 of example/review-bot on GitHub (opens in a new tab)' }))
      .toHaveAttribute('href', 'https://github.com/example/review-bot/commit/3333333333333333333333333333333333333333');
    await expect(page.locator('#without-skills-summary')).toHaveText('2 repositories without skills');
    await expect(page.locator('#skill-count')).toHaveText('3');
    await expect(page.locator('#similar-group-count')).toHaveText('1');
    await expect(page.locator('#largest-group-count')).toHaveText('2');
    // Matching names group skills from different repositories.
    await expect(page.locator('.skill-group h3')).toHaveText('code-review');
    await expect(page.locator('.skill-group .group-size')).toHaveText('2 skills');
    await expect(page.locator('#skill-list > .skill-card .skill-path')).toHaveText('example/agent-tools/skills/release-notes/SKILL.md');
    // Organization scans do not add entries to recent repositories.
    await expect(page.locator('#recent-status')).toHaveText('No recently scanned repositories yet.');
    await checkpoint('03-organization-results');

    await page.locator('#without-skills-summary').click();
    await expect(page.locator('#without-skills-list > li')).toHaveText(['example/docs-site', 'example/sandbox (empty)']);
    await page.getByRole('searchbox', { name: 'Search skills' }).fill('example/review-bot/');
    await expect(page.locator('#result-count')).toHaveText('Showing 1 of 3 skills in 1 of 2 groups (including standalone skills) · largest first');
    await expect(page.locator('.group-details')).toHaveAttribute('open', '');
    await expect(page.locator('.group-details summary')).toHaveText('Showing 1 of 2 skills matching filters');
    await expect(page.locator('.group-members .skill-path')).toHaveText('example/review-bot/.agents/skills/review/SKILL.md');
    await expect(page.locator('#similar-group-count')).toHaveText('1');
    await checkpoint('04-filter-by-repository');

    // A rate limit stops the whole scan instead of reporting every later repository.
    await scan.click();
    await scanning(2);
    await mock.complete();
    await expect(scan).toBeEnabled();
    await expect(input).toBeEnabled();
    await expect(page.locator('#error')).toBeVisible();
    await expect(page.locator('#error')).toBeFocused();
    await expect(page.locator('#error-message')).toHaveText(/^Stopped at example\/docs-site \(repository 2 of 5\): GitHub denied access or its API rate limit was reached; .*GITHUB_TOKEN.*\(HTTP 403\)$/);
    await expect(page.locator('#results-status')).toHaveText('Scan incomplete');
    await expect(page.locator('#scan-results')).toBeHidden();
    await expect(page.locator('#search')).toHaveValue('');
    await checkpoint('05-rate-limit-stops-scan');

    // Selecting a saved or example repository returns to repository mode.
    await page.getByRole('button', { name: 'Repository', exact: true }).click();
    await expect(page.getByRole('textbox', { name: 'GitHub repository' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Scan repository', exact: true })).toBeEnabled();
    await expect(page.locator('#scan-title')).toHaveText('Start with a repository');

    mock.assertFinished();
    expect(errors).toEqual([]);
  } finally {
    await app.stop();
  }
});
