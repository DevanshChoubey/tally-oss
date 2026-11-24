// Compares the app's captures with the mock's and writes COMPARE.md next to them.
//
//   node design/compare.mjs                 reads the folder this file is in
//   node design/compare.mjs <design dir>    reads another folder with the same layout (tests)
//   node design/compare.mjs --no-score      skips the SSIM scores (faster)
//
// Inputs, under the design folder:
//   mock-v2/states.json                          the mock states and themes
//   mock-v2/captures/states/<theme>/<id>.webp   the mock captures
//   app-captures/<theme>/<id>.webp              the app captures (capture.sh writes them)
// Output: COMPARE.md in the design folder, with paths relative to it.
//
// Scores use ffmpeg's ssim filter, so no npm packages are needed. SSIM is 1 for identical images
// and falls as they differ. The app image is scaled to the mock's size first. Sections run worst
// first (lowest SSIM); an app capture that cannot be scored is listed last.

import { spawnSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const args = process.argv.slice(2);
const noScore = args.includes('--no-score');
const positional = args.filter((a) => !a.startsWith('--'));
const design = positional[0] ? resolve(positional[0]) : dirname(fileURLToPath(import.meta.url));

const mockRoot = join(design, 'mock-v2');
const appRoot = join(design, 'app-captures');
const states = JSON.parse(readFileSync(join(mockRoot, 'states.json'), 'utf8'));

const mockFile = (theme, id) => join(mockRoot, 'captures', 'states', theme, `${id}.webp`);
const appFile = (theme, id) => join(appRoot, theme, `${id}.webp`);
const mockPath = (theme, id) => `mock-v2/captures/states/${theme}/${id}.webp`;
const appPath = (theme, id) => `app-captures/${theme}/${id}.webp`;

/** Every app capture on disk as { theme, id }. */
function appCaptures() {
  const found = [];
  if (!existsSync(appRoot)) return found;
  for (const theme of readdirSync(appRoot).sort()) {
    const dir = join(appRoot, theme);
    if (!statSync(dir).isDirectory()) continue;
    // The audit set has no mock states of the same ids (design/REAL-PATH-AUDIT.md judges it).
    if (theme === 'audit') continue;
    for (const file of readdirSync(dir).sort()) {
      if (file.endsWith('.webp')) found.push({ theme, id: file.slice(0, -'.webp'.length) });
    }
  }
  return found;
}

/** The pixel size of an image, read from ffmpeg's probe output, or null. */
function size(file) {
  const r = spawnSync('ffmpeg', ['-hide_banner', '-i', file], { encoding: 'utf8' });
  if (r.error) throw r.error;
  const m = /Video:.*?\b(\d{2,5})x(\d{2,5})\b/.exec(r.stderr);
  return m ? { w: Number(m[1]), h: Number(m[2]) } : null;
}

/** SSIM of the app image against the mock image (the app image scaled to the mock's size), or null. */
function ssim(mock, app) {
  const dims = size(mock);
  if (!dims || !size(app)) return null;
  const filter = `[1:v]scale=${dims.w}:${dims.h}:flags=bicubic[b];[0:v][b]ssim`;
  const r = spawnSync('ffmpeg', ['-hide_banner', '-nostats', '-i', mock, '-i', app,
    '-lavfi', filter, '-f', 'null', '-'], { encoding: 'utf8' });
  if (r.error) throw r.error;
  if (r.status !== 0) return null;
  const last = [...r.stderr.matchAll(/All:([0-9.]+)/g)].at(-1);
  return last ? Number(last[1]) : null;
}

// Pair every app capture that has a mock capture of the same theme and state id.
const pairs = [];
const appOnly = [];
for (const { theme, id } of appCaptures()) {
  if (!existsSync(mockFile(theme, id))) {
    appOnly.push(`${theme}/${id}`);
    continue;
  }
  pairs.push({ theme, id, score: noScore ? null : ssim(mockFile(theme, id), appFile(theme, id)) });
}
pairs.sort((a, b) => {
  if (a.score === null || b.score === null) return (a.score === null) - (b.score === null);
  return a.score - b.score || `${a.theme}/${a.id}`.localeCompare(`${b.theme}/${b.id}`);
});

// Mock states, per theme, that have a mock capture but no app capture yet.
const missing = new Map();
for (const theme of states.themes) {
  const ids = states.states
    .map((s) => s.id)
    .filter((id) => existsSync(mockFile(theme, id)) && !existsSync(appFile(theme, id)));
  missing.set(theme, ids);
}

const fmt = (score) => (score === null ? 'not scored' : score.toFixed(4));
const out = [];
out.push('# App against mock', '');
out.push(`${pairs.length} app captures are paired with a mock capture of the same theme and state id.`
  + (noScore ? ' Scores were skipped (`--no-score`).' : ' SSIM is 1 for identical images; lower is a bigger difference, and the list runs worst first.'), '');

if (pairs.length > 0) {
  out.push('| Theme | State | SSIM |', '| --- | --- | --- |');
  for (const p of pairs) out.push(`| ${p.theme} | ${p.id} | ${fmt(p.score)} |`);
  out.push('');
}

for (const p of pairs) {
  out.push(`## ${p.theme} / ${p.id}`, '', `SSIM ${fmt(p.score)}`, '',
    '<table>',
    '<tr><th>Mock</th><th>App</th></tr>',
    '<tr>',
    `<td><img src="${mockPath(p.theme, p.id)}" width="480" alt="mock ${p.theme} ${p.id}"></td>`,
    `<td><img src="${appPath(p.theme, p.id)}" width="480" alt="app ${p.theme} ${p.id}"></td>`,
    '</tr>',
    '</table>', '');
}

out.push('## Mock states with no app capture yet', '');
for (const [theme, ids] of missing) {
  out.push(`### ${theme} (${ids.length})`, '');
  if (ids.length === 0) out.push('None.');
  for (const id of ids) out.push(`- \`${id}\``);
  out.push('');
}

if (appOnly.length > 0) {
  out.push('## App captures with no mock capture', '');
  for (const label of appOnly) out.push(`- \`${label}\``);
  out.push('');
}

const target = join(design, 'COMPARE.md');
writeFileSync(target, `${out.join('\n')}\n`);
const scored = pairs.filter((p) => p.score !== null);
const worst = scored[0];
const pending = [...missing.values()].reduce((n, ids) => n + ids.length, 0);
console.log(`wrote ${target}: ${pairs.length} pairs`
  + (worst ? `, worst ${worst.theme}/${worst.id} at SSIM ${worst.score.toFixed(4)}` : '')
  + `, ${pending} mock states without an app capture`);
