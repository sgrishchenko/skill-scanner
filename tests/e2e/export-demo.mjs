import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

// Each demo is recorded from its spec file's single test.
const demos = { 'organization-scan': 5, 'recent-repositories': 9, 'starred-skills': 9 };
const name = process.argv[2];
if (!(name in demos)) throw new Error(`Usage: npm run demo -- <${Object.keys(demos).join('|')}>`);
const directory = fileURLToPath(new URL('.', import.meta.url));
const report = JSON.parse(readFileSync(path.join(directory, 'test-results/report.json')));
const specs = (suites) => suites.flatMap((suite) => [...suite.specs, ...specs(suite.suites ?? [])]);
const all = specs(report.suites).flatMap((spec) => spec.tests.map((test) => ({ ...test, file: spec.file })));
const tests = all.filter((test) => test.file === `${name}.spec.mjs`);
const result = tests[0]?.results[0];
if (report.config.updateSnapshots !== 'none' || !report.config.metadata.clean ||
    report.stats.expected !== all.length || report.stats.unexpected || report.stats.flaky || report.stats.skipped ||
    report.errors.length || tests.length !== 1 || tests[0].results.length !== 1 || result?.status !== 'passed') {
  throw new Error(`Record a run where every test, including ${name}, passes from a clean commit with baseline updates disabled before exporting.`);
}
const video = result.attachments.find((attachment) => attachment.name === 'video');
if (!video?.path) throw new Error('The passing run has no video attachment.');
// Reports produced in /work inside Docker are also exportable on the host.
const recording = path.resolve(directory, path.relative(report.config.rootDir, video.path));
const checkpoints = tests[0].annotations.filter((annotation) => annotation.type === 'checkpoint').map((annotation) => annotation.description);
if (checkpoints.length !== demos[name]) throw new Error(`The passing run must include all ${demos[name]} screenshot checkpoints.`);
const output = fileURLToPath(new URL(`../../docs/demos/${name}`, import.meta.url));
execFileSync('ffmpeg', [
  '-hide_banner', '-loglevel', 'error', '-y', '-i', recording,
  '-an', '-c:v', 'libx264', '-preset', 'slow', '-crf', '22', '-pix_fmt', 'yuv420p',
  '-movflags', '+faststart', `${output}.mp4`,
], { stdio: 'inherit' });
execFileSync('ffmpeg', [
  '-hide_banner', '-loglevel', 'error', '-y', '-i', recording,
  '-filter_complex', 'fps=8,scale=960:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse',
  '-loop', '0', `${output}.gif`,
], { stdio: 'inherit' });
writeFileSync(`${output}.json`, JSON.stringify({
  tested_revision: report.config.metadata.revision,
  environment: report.config.metadata.environment,
  result: 'passed',
  baseline_updates: 'none',
  checkpoints,
  sha256: Object.fromEntries(['mp4', 'gif'].map((extension) => [extension,
    createHash('sha256').update(readFileSync(`${output}.${extension}`)).digest('hex'),
  ])),
}, null, 2) + '\n');
console.log(`Exported the passing ${name} run at ${report.config.metadata.revision} to docs/demos/.`);
