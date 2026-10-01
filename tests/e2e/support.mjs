import { spawn, execFileSync } from 'node:child_process';
import { once } from 'node:events';
import { mkdir, readFile, rename, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect } from '@playwright/test';

const root = fileURLToPath(new URL('../../', import.meta.url));
const binary = path.join(root, 'target/debug/skill-scanner');

export class LocalApp {
  constructor(testInfo) {
    this.history = testInfo.outputPath('state', 'recent');
    this.codexDirectory = testInfo.outputPath('state', 'codex');
    this.env = {
      ...process.env,
      SKILL_SCANNER_HISTORY_DIR: this.history,
      SKILL_SCANNER_STARRED_DIR: testInfo.outputPath('state', 'starred'),
      SKILL_SCANNER_CODEX_SKILLS_DIR: this.codexDirectory,
      SKILL_SCANNER_CACHE_DIR: testInfo.outputPath('state', 'cache'),
      // Browser scan requests are intercepted. Also prevent an accidental
      // server-side request from reaching GitHub if that interception regresses.
      HTTPS_PROXY: 'http://127.0.0.1:9',
      HTTP_PROXY: 'http://127.0.0.1:9',
      ALL_PROXY: 'http://127.0.0.1:9',
      NO_PROXY: '127.0.0.1,localhost',
    };
    delete this.env.GITHUB_TOKEN;
    delete this.env.GH_TOKEN;
    for (const key of ['https_proxy', 'http_proxy', 'all_proxy', 'no_proxy']) {
      delete this.env[key];
    }
  }

  async start() {
    this.server = spawn(binary, ['serve', '--port', '0'], {
      env: this.env, stdio: ['ignore', 'ignore', 'pipe'],
    });
    await new Promise((resolve, reject) => {
      let stderr = '';
      const timeout = setTimeout(() => reject(new Error(`Server startup timed out: ${stderr}`)), 10_000);
      const finish = (error) => {
        clearTimeout(timeout);
        if (error) reject(error);
        else resolve();
      };
      this.server.once('error', finish);
      this.server.once('exit', (code) => finish(new Error(`Server exited (${code}): ${stderr}`)));
      this.server.stderr.on('data', (data) => {
        stderr += data;
        const match = stderr.match(/available at (http:\/\/127\.0\.0\.1:\d+)/);
        if (match) {
          this.url = match[1];
          finish();
        }
      });
    });
  }

  async stop() {
    if (!this.server || this.server.exitCode !== null || this.server.signalCode !== null) return;
    const stopped = once(this.server, 'exit');
    this.server.kill('SIGTERM');
    const timeout = setTimeout(() => this.server.kill('SIGKILL'), 5_000);
    try { await stopped; } finally { clearTimeout(timeout); }
  }

  recent() {
    return this.cli('recent');
  }

  starred() {
    return this.cli('starred');
  }

  codex() {
    return this.cli('codex');
  }

  cli(...args) {
    return execFileSync(binary, args, { env: this.env, encoding: 'utf8', timeout: 10_000 });
  }
}

export async function checkpoint(page, testInfo, name) {
  await page.locator('.scan-panel').evaluate((panel) => window.scrollTo(0, panel.offsetTop - 24));
  await page.mouse.move(0, 0);
  await expect(page).toHaveScreenshot(`${name}.png`, { fullPage: true });
  testInfo.annotations.push({ type: 'checkpoint', description: name });
  // Assertions before each checkpoint determine readiness. This pause only
  // makes the same passing test's video readable; it never synchronizes the UI.
  await page.waitForTimeout(1_200);
}

// Intercept one scan route, holding each response until the test has asserted
// the scanning state. The other scan route and external requests are failures.
async function interceptScans(context, app, pathname, fixtureFile) {
  const fixtures = JSON.parse(await readFile(new URL(`fixtures/${fixtureFile}`, import.meta.url)));
  const other = pathname === '/api/scan' ? '/api/scan-org' : '/api/scan';
  const pending = [];
  const requests = [];
  const unexpected = [];
  const state = { completed: 0 };

  await context.route('**/*', async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    if (url.origin !== app.url || url.pathname === other) {
      unexpected.push(request.url());
      await route.abort('blockedbyclient');
    } else if (url.pathname === pathname) {
      requests.push(request);
      pending.push(route);
    } else {
      await route.continue();
    }
  });

  return {
    requests,
    unexpected,
    async next(body) {
      await expect.poll(() => pending.length).toBe(1);
      const route = pending.shift();
      const fixture = fixtures[state.completed++];
      expect(fixture, 'unexpected extra scan').toBeDefined();
      expect(route.request().method()).toBe('POST');
      expect(route.request().headers()['x-skill-scanner']).toBe('1');
      expect(route.request().postDataJSON()).toEqual(body(fixture));
      return { route, fixture };
    },
    assertFinished() {
      expect(state.completed).toBe(fixtures.length);
      expect(requests).toHaveLength(fixtures.length);
      expect(pending).toHaveLength(0);
      expect(unexpected).toEqual([]);
    },
  };
}

function ndjson(events) {
  return events.map((event) => JSON.stringify(event)).join('\n') + '\n';
}

export async function mockScans(context, app, fixtureFile = 'scans.json') {
  const scans = await interceptScans(context, app, '/api/scan', fixtureFile);
  return {
    requests: scans.requests,
    unexpected: scans.unexpected,
    assertFinished: scans.assertFinished,
    async complete() {
      const { route, fixture } = await scans.next((fixture) => ({ repository: fixture.repository }));
      const commit = '1111111111111111111111111111111111111111';
      const skills = fixture.skills.map((skill) => ({
        ...skill, warnings: [],
        link: `https://github.com/${fixture.repository}/blob/${commit}/${skill.path}`,
      }));

      // Only scan output and its history side effect are mocked. Listing,
      // removal, restarts, and the CLI exercise the real binary and disk state.
      // Rust tests independently cover scan_with_storage recording semantics.
      await mkdir(app.history, { recursive: true });
      const [owner, name] = fixture.repository.split('/');
      const file = path.join(app.history, `owner-${owner}_repo-${name}.json`);
      await writeFile(`${file}.tmp`, JSON.stringify({
        format_version: 1, repository: fixture.repository,
        scanned_at: fixture.scanned_at, skill_count: skills.length,
      }));
      await rename(`${file}.tmp`, file);
      const inventory = {
        repository: fixture.repository, commit, skills,
        aggregation: {
          groups: skills.map((_, index) => ({ skill_indices: [index], skills_with_warnings: 0 })),
          statistics: {
            total_skills: skills.length, similar_groups: 0, grouped_skills: 0,
            standalone_skills: skills.length, largest_group: 0, skills_with_warnings: 0,
          },
        },
      };
      const events = [
        { type: 'progress', message: `Resolving repository: ${fixture.repository}`, current: null, total: null },
        { type: 'complete', inventory },
      ];
      await route.fulfill({ contentType: 'application/x-ndjson', body: ndjson(events) });
    },
  };
}

// Organization fixtures contain the complete event stream. Organization scans
// never record recent repositories, so no history is written.
export async function mockOrganizationScans(context, app) {
  const scans = await interceptScans(context, app, '/api/scan-org', 'organization-scans.json');
  return {
    requests: scans.requests,
    unexpected: scans.unexpected,
    assertFinished: scans.assertFinished,
    async complete() {
      const { route, fixture } = await scans.next((fixture) => ({ organization: fixture.organization }));
      await route.fulfill({ contentType: 'application/x-ndjson', body: ndjson(fixture.events) });
    },
  };
}

// Installing downloads files from GitHub, so only the listed installs are
// mocked: each response is held until the test has asserted the pending state,
// and the installed folder is written as the server would write it. Other
// install requests, such as name conflicts, reach the actual server, and
// listing, removal, restarts, and the CLI use the real binary and disk state.
// Rust tests independently cover downloading and publishing installations.
export async function mockCodexInstalls(context, app, installs) {
  const pending = [];
  const requests = [];
  await context.route((url) => url.pathname === '/api/codex/install', async (route) => {
    const body = route.request().postDataJSON();
    requests.push(body);
    if (installs.some((install) => install.repository === body.repository && install.path === body.path)) {
      pending.push(route);
    } else {
      await route.fallback();
    }
  });
  return {
    requests,
    async complete(install) {
      await expect.poll(() => pending.length).toBe(1);
      const route = pending.shift();
      const request = route.request();
      expect(request.method()).toBe('POST');
      expect(request.headers()['x-skill-scanner']).toBe('1');
      expect(request.postDataJSON()).toEqual({ repository: install.repository, path: install.path, commit: install.commit });
      const folder = path.join(app.codexDirectory, install.name);
      await mkdir(folder, { recursive: true });
      await writeFile(path.join(folder, 'SKILL.md'), `---\nname: ${install.name}\ndescription: ${install.description}\n---\n`);
      await writeFile(path.join(folder, '.skill-scanner.json'), JSON.stringify({
        format_version: 1, repository: install.repository, path: install.path,
        commit: install.commit, installed_at: install.installed_at,
      }));
      const skill = {
        name: install.name, repository: install.repository, path: install.path, commit: install.commit,
        link: `https://github.com/${install.repository}/blob/${install.commit}/${install.path}`,
        installed_at: install.installed_at,
      };
      await route.fulfill({ contentType: 'application/json', body: JSON.stringify({ skill, files: 1 }) });
    },
    assertFinished() {
      expect(pending).toHaveLength(0);
    },
  };
}
