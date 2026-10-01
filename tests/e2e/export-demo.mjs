import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const directory = fileURLToPath(new URL('.', import.meta.url));
// Export one spec's recording: `npm run demo -- organization-scan`.
const demo = process.argv[2] ?? 'recent-repositories';
const baselines = readdirSync(path.join(directory, 'snapshots', `${demo}.spec.mjs`))
  .filter((file) => file.endsWith('.png')).map((file) => file.slice(0, -4)).sort();
const report = JSON.parse(readFileSync(path.join(directory, 'test-results/report.json')));
const specs = (suites) => suites.flatMap((suite) => [...suite.specs, ...specs(suite.suites ?? [])]);
const recorded = specs(report.suites);
const tests = recorded.flatMap((spec) => spec.tests);
const result = tests[0]?.results[0];
if (report.config.updateSnapshots !== 'none' || !report.config.metadata.clean ||
    report.stats.expected !== 1 || report.stats.unexpected || report.stats.flaky || report.stats.skipped ||
    report.errors.length || tests.length !== 1 || tests[0].results.length !== 1 || result?.status !== 'passed' ||
    recorded.length !== 1 || recorded[0].file !== `${demo}.spec.mjs`) {
  throw new Error(`Record one passing ${demo} test from a clean commit with baseline updates disabled before exporting.`);
}
const video = result.attachments.find((attachment) => attachment.name === 'video');
if (!video?.path) throw new Error('The passing run has no video attachment.');
// Reports produced in /work inside Docker are also exportable on the host.
const recording = path.resolve(directory, path.relative(report.config.rootDir, video.path));
const checkpoints = tests[0].annotations.filter((annotation) => annotation.type === 'checkpoint').map((annotation) => annotation.description);
if (JSON.stringify(checkpoints) !== JSON.stringify(baselines)) {
  throw new Error(`The passing run must include every screenshot checkpoint: ${baselines.join(', ')}.`);
}
const output = fileURLToPath(new URL(`../../docs/demos/${demo}`, import.meta.url));
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
console.log(`Exported the passing run at ${report.config.metadata.revision} to docs/demos/.`);
