// Captures every state and transition of the mock as lossless WebP images, and each transition as a GIF.
//
//   node capture.mjs            capture everything into captures/
//   node capture.mjs --only bank   only state or transition ids containing "bank"
//   node capture.mjs --check    exit 1 when captures/ is older than index.html
//
// Follows the capture contract in DESIGN.md, section 8. It reads states.json and transitions.json,
// loads each URL fresh in headless Chrome, waits for the page's ready flag, and screenshots the
// 1280x800 viewport.
// Needs a Chrome and an ffmpeg built with libwebp. GIFs are built with ffmpeg from the transition's frames.
//
// It needs google-chrome and ffmpeg on PATH, and no npm packages: Node's built-in WebSocket talks
// to Chrome's DevTools protocol.

import { spawn, execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync, existsSync, rmSync, mkdtempSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const out = join(here, 'captures');
const page = pathToFileURL(join(here, 'index.html')).href;
const states = JSON.parse(readFileSync(join(here, 'states.json'), 'utf8'));
const transitions = JSON.parse(readFileSync(join(here, 'transitions.json'), 'utf8'));
const args = process.argv.slice(2);
const only = args.includes('--only') ? args[args.indexOf('--only') + 1] : null;

/** The sha256 of the files the captures are made from. */
function sourceHash() {
  const h = createHash('sha256');
  for (const f of ['index.html', 'states.json', 'transitions.json']) {
    h.update(readFileSync(join(here, f)));
  }
  return h.digest('hex');
}

if (args.includes('--check')) {
  const manifest = join(out, 'manifest.json');
  const stale = !existsSync(manifest)
    || JSON.parse(readFileSync(manifest, 'utf8')).source_sha256 !== sourceHash();
  console.log(stale ? 'captures are stale: run node capture.mjs' : 'captures are current');
  process.exit(stale ? 1 : 0);
}

// ---- Chrome over the DevTools protocol -------------------------------------------------------

const profile = mkdtempSync(join(process.env.TMPDIR || here, 'ledgerly-capture-'));
// Spell checking off for the whole profile: its dictionary loads in the background and its
// squiggles land on some captures and not others, whatever the page's spellcheck attribute says.
mkdirSync(join(profile, 'Default'));
writeFileSync(join(profile, 'Default', 'Preferences'),
  JSON.stringify({ browser: { enable_spellchecking: false }, spellcheck: { dictionaries: [] } }));
const chrome = spawn('google-chrome', [
  '--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`,
  '--no-first-run', '--no-default-browser-check', '--hide-scrollbars',
  '--force-color-profile=srgb', '--allow-file-access-from-files',
  // Software raster on one thread: GPU and partial raster antialias rounded corners differently
  // from run to run.
  '--disable-gpu', '--disable-partial-raster', '--disable-skia-runtime-opts', '--num-raster-threads=1',
  'about:blank',
], { stdio: ['ignore', 'ignore', 'pipe'] });

const wsUrl = await new Promise((resolve, reject) => {
  let buf = '';
  chrome.stderr.on('data', (d) => {
    buf += d;
    const m = buf.match(/DevTools listening on (ws:\/\/\S+)/);
    if (m) resolve(m[1]);
  });
  chrome.on('exit', (code) => reject(new Error(`chrome exited with ${code}: ${buf}`)));
});

const browser = new WebSocket(wsUrl);
await new Promise((r) => browser.addEventListener('open', r, { once: true }));
let nextId = 1;
const pending = new Map();
browser.addEventListener('message', (e) => {
  const msg = JSON.parse(e.data);
  if (msg.id && pending.has(msg.id)) {
    const { resolve, reject } = pending.get(msg.id);
    pending.delete(msg.id);
    if (msg.error) reject(new Error(JSON.stringify(msg.error)));
    else resolve(msg.result);
  }
});
function send(method, params = {}, sessionId) {
  const id = nextId++;
  browser.send(JSON.stringify({ id, method, params, sessionId }));
  return new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
}

const { targetId } = await send('Target.createTarget', { url: 'about:blank' });
const { sessionId } = await send('Target.attachToTarget', { targetId, flatten: true });
const cdp = (method, params) => send(method, params, sessionId);
await cdp('Page.enable');
await cdp('Runtime.enable');
const { width, height } = states.viewport;
await cdp('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Loads `url` fresh, waits for the ready flag (bounded), and saves a screenshot to `file`. */
async function shoot(url, file) {
  await cdp('Page.navigate', { url: 'about:blank' });
  await cdp('Page.navigate', { url });
  const start = Date.now();
  for (;;) {
    const { result } = await cdp('Runtime.evaluate', { expression: states.ready, returnByValue: true });
    if (result.value === true) break;
    if (Date.now() - start > 15000) throw new Error(`not ready after 15 s: ${url}`);
    await sleep(50);
  }
  // The two things that move on a settled page: the blinking text caret and the looping skeleton
  // shimmer. Stop both, so every capture of a state has the same pixels.
  await cdp('Runtime.evaluate', { expression: `{
    const s = document.createElement('style');
    s.textContent = '*{caret-color:transparent!important} body:not(.frozen) .skel{animation:none!important}';
    document.head.append(s);
    document.documentElement.spellcheck = false; // the spellchecker's squiggles arrive at random times
  }` });
  // Shoot until two shots in a row are the same, so nothing still settling ends up in the image.
  let data = (await cdp('Page.captureScreenshot', { format: 'png' })).data;
  for (let tries = 0; ; tries++) {
    await sleep(100);
    const again = (await cdp('Page.captureScreenshot', { format: 'png' })).data;
    if (again === data) break;
    if (tries === 20) throw new Error(`still changing after 2 s: ${url}`);
    data = again;
  }
  mkdirSync(dirname(file), { recursive: true });
  // Lossless WebP: half the size of PNG, the same pixels, and the same bytes for the same render,
  // so git stores an unchanged screen once.
  const png = join(profile, 'shot.png');
  writeFileSync(png, Buffer.from(data, 'base64'));
  execFileSync('ffmpeg', ['-loglevel', 'error', '-y', '-i', png,
    '-c:v', 'libwebp', '-lossless', '1', '-compression_level', '6', '-map_metadata', '-1', file]);
}

const fill = (template, values) => template.replace(/\{(\w+)\}/g, (_, k) => values[k]);
const urlOf = (template, values) => `${page}${fill(template, values).replace(/^index\.html/, '')}`;
const wanted = (id) => !only || id.includes(only);

// ---- capture -----------------------------------------------------------------------------------

const errors = [];
let count = 0;
if (!only) rmSync(out, { recursive: true, force: true });

for (const s of states.states.filter((x) => wanted(x.id))) {
  for (const theme of states.themes) {
    try {
      await shoot(urlOf(states.url, { hash: s.hash, theme }), join(out, 'states', theme, `${s.id}.webp`));
      count++;
    } catch (e) { errors.push(`${s.id} (${theme}): ${e.message}`); }
  }
  process.stdout.write(`\rstates: ${count}`);
}
console.log();

for (const t of transitions.transitions.filter((x) => wanted(x.id))) {
  const hash = t.render === 'start' ? t.start : t.end;
  for (const theme of transitions.themes) {
    const dir = join(out, 'transitions', t.id, theme);
    try {
      for (const ms of t.frames_ms) {
        await shoot(urlOf(transitions.url, { ms, render_hash: hash, id: t.id, theme }),
          join(dir, `${String(ms).padStart(4, '0')}ms.webp`));
        count++;
      }
      gif(t, dir, join(out, 'transitions', t.id, `${theme}.gif`));
    } catch (e) { errors.push(`${t.id} (${theme}): ${e.message}`); }
  }
  process.stdout.write(`\rtransitions: ${t.id}            `);
}
console.log();

/** A GIF of the frames, each held for the gap to the next frame, the last held for 600 ms. */
function gif(t, dir, file) {
  const lines = [];
  t.frames_ms.forEach((ms, i) => {
    const next = t.frames_ms[i + 1];
    const hold = next === undefined ? 600 : Math.max(next - ms, 20);
    lines.push(`file '${join(dir, `${String(ms).padStart(4, '0')}ms.webp`)}'`, `duration ${hold / 1000}`);
  });
  lines.push(`file '${join(dir, `${String(t.frames_ms.at(-1)).padStart(4, '0')}ms.webp`)}'`);
  const list = join(profile, `${t.id}.txt`);
  writeFileSync(list, lines.join('\n'));
  execFileSync('ffmpeg', ['-loglevel', 'error', '-y', '-f', 'concat', '-safe', '0', '-i', list,
    '-vf', 'scale=960:-1:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse',
    '-loop', '0', file]);
}

// Chrome writes to its profile until it exits, so wait for the exit before removing the profile.
const exited = new Promise((r) => chrome.once('exit', r));
await send('Browser.close').catch(() => {});
await Promise.race([exited, sleep(5000)]);
chrome.kill();
rmSync(profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });

if (!only) {
  writeFileSync(join(out, 'manifest.json'), JSON.stringify({
    source_sha256: sourceHash(), images: count, errors,
    states: states.states.length, transitions: transitions.transitions.length, themes: states.themes,
  }, null, 1));
  execFileSync(process.execPath, [join(here, 'catalog.mjs')], { stdio: 'inherit' });
}
console.log(`${count} images, ${errors.length} errors`);
for (const e of errors) console.log(`  ${e}`);
process.exit(errors.length ? 1 : 0);
