// Record real keyboard input in the site's existing WASM editor; no app rebuild.
// Start the built site locally, then run with Playwright available in NODE_PATH:
//   node scripts/capture-editor-demo.cjs http://127.0.0.1:5500 /tmp/editor-demo
// CHROME_EXECUTABLE may point to an installed Chrome instead of Playwright's browser.
const { chromium } = require('playwright');
const fs = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');

const origin = process.argv[2] || 'http://127.0.0.1:5500';
const output = process.argv[3] || '/tmp/editor-demo';
const width = 420;
const height = 520;

async function record(browser, theme) {
  const directory = path.join(output, theme);
  await fs.mkdir(directory, { recursive: true });
  const context = await browser.newContext({
    viewport: { width, height }, deviceScaleFactor: 1,
    colorScheme: theme, permissions: ['clipboard-read', 'clipboard-write'],
  });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.route('**/__editor-demo-recording', route => route.fulfill({
    contentType: 'text/html',
    body: `<!doctype html><html data-lb-mode="${theme}"><meta charset="utf-8">
      <style>html,body{margin:0;overflow:hidden}#editor-demo{display:block;width:${width}px;height:${height}px}
      #canvas-demo{position:absolute;top:900px;width:${width}px;height:${height}px}
      #capture-pointer{position:fixed;left:0;top:0;width:25px;height:31px;z-index:10;display:none;pointer-events:none;filter:drop-shadow(0 1px 1px #0005)}
      #capture-pointer circle{opacity:0}#capture-pointer[data-pressed="true"] circle{opacity:1}</style>
      <canvas id="editor-demo" tabindex="0"></canvas><div id="editor-loading"></div>
      <canvas id="canvas-demo" tabindex="0"></canvas><div id="canvas-loading"></div>
      <svg id="capture-pointer" viewBox="0 0 25 31" overflow="visible" aria-hidden="true">
        <circle cx="1" cy="1" r="14" fill="#4aa3ff55" stroke="#268aff" stroke-width="1.5"/>
        <path d="M1 1L1 23L7 17L12 28L17 26L12 15L21 15Z" fill="white" stroke="#171717" stroke-width="1.5" stroke-linejoin="round"/>
      </svg>
      <script>
        const pointer = document.getElementById('capture-pointer');
        document.addEventListener('pointermove', e => { pointer.style.transform = 'translate(' + e.clientX + 'px,' + e.clientY + 'px)'; }, true);
        document.addEventListener('pointerdown', () => { pointer.dataset.pressed = 'true'; }, true);
        document.addEventListener('pointerup', () => { pointer.dataset.pressed = 'false'; }, true);
      </script>
      <script type="module">import init from '/wasm/public-site.js'; await init('/wasm/public-site_bg.wasm');</script></html>`,
  }));
  await page.goto(`${origin}/__editor-demo-recording`);
  await page.waitForSelector('#editor-loading', { state: 'detached', timeout: 60000 });
  await page.waitForTimeout(600);
  const modifier = await page.evaluate(() => /Mac/.test(navigator.platform) ? 'Meta' : 'Control');
  await page.mouse.click(210, 190);
  await page.keyboard.press(`${modifier}+a`);
  await page.waitForTimeout(150);
  await page.keyboard.press('Backspace');
  await page.waitForTimeout(150);
  await page.mouse.move(419, 399);

  const frames = [];
  async function frame(duration = 70) {
    // Allow the WASM event loop to apply each keystroke before reading pixels.
    await page.waitForTimeout(45);
    const file = `frame-${String(frames.length).padStart(4, '0')}.png`;
    await page.screenshot({ path: path.join(directory, file) });
    frames.push({ file, duration });
  }
  async function type(text, duration = 70) {
    for (const character of text) {
      await page.keyboard.type(character);
      await frame(duration);
    }
  }
  async function key(value, duration = 180) {
    await page.keyboard.press(value);
    await frame(duration);
  }
  async function paste(text) {
    await page.evaluate(async value => {
      window.recordedPaste = null;
      document.addEventListener('paste', event => {
        window.recordedPaste = {
          text: event.clipboardData.getData('text/plain'), trusted: event.isTrusted,
        };
      }, { once: true, capture: true });
      await navigator.clipboard.writeText(value);
    }, text);
    await page.keyboard.press(`${modifier}+v`);
    await page.waitForFunction(() => window.recordedPaste !== null);
    assert.deepEqual(await page.evaluate(() => window.recordedPaste), { text, trusted: true });
    await frame(350);
  }
  function hold(milliseconds) { frames[frames.length - 1].duration += milliseconds; }

  const syntax = text => type(text, 150);
  const bracketUrl = 'https://www.ikea.com/us/en/cat/brackets-20659/';
  await frame(500);
  await syntax('# ');
  await type('todo');
  await key('Enter');
  await key('Enter');
  await syntax('## ');
  await type('work');
  await key('Enter');
  await syntax('- [ ] ');
  await type('fix the login bug');
  await key('Enter');
  await key('Enter');
  await syntax('```');
  await type('sh');
  await key('Enter');
  await type('cargo test login');
  await key('Enter');
  await syntax('```');
  await key('Enter');
  await key('Enter');
  hold(300);
  await syntax('## ');
  await type('errands');
  await key('Enter');
  await syntax('- [ ] **');
  await type('amazon returns');
  await syntax('**');
  await key('Enter', 350);
  await type('buy ');
  await syntax('[');
  await type('shelf brackets');
  await syntax('](');
  await paste(bracketUrl);
  await syntax(')');
  await key('Enter', 350);
  await key('Enter');
  await key('Enter');
  await syntax('## ');
  await type('home');
  await key('Enter');
  await syntax('- [ ] ');
  await type('fix doorbell');
  await key('Enter', 350);
  await type('put up shelf');
  await key('ArrowUp', 600);
  await page.screenshot({ path: path.join(directory, 'before-interactions.png') });

  // Browser screenshots omit the OS pointer. This recording-only overlay follows
  // the real pointer events, including the pressed state; it never intercepts input.
  await page.evaluate(() => { document.getElementById('capture-pointer').style.display = 'block'; });
  let pointer = { x: 419, y: 399 };
  async function move(x, y, count = 12) {
    const start = pointer;
    for (let i = 1; i <= count; i++) {
      const t = i / count;
      const ease = t * t * (3 - 2 * t);
      await page.mouse.move(start.x + (x - start.x) * ease, start.y + (y - start.y) * ease);
      await frame(60);
    }
    pointer = { x, y };
  }
  async function click() {
    await page.mouse.down();
    await frame(150);
    await page.mouse.up();
    await page.waitForTimeout(200);
    await frame(450);
  }
  // Lift "put up shelf" by its checkbox and move it above "fix doorbell".
  await move(51, 463);
  hold(500);
  await page.mouse.down();
  await frame(180);
  await move(53, 430, 18);
  hold(450);
  await page.screenshot({ path: path.join(directory, 'dragging.png') });
  await page.mouse.up();
  await page.waitForTimeout(250);
  await frame(650);
  await page.screenshot({ path: path.join(directory, 'reordered.png') });

  // Deliberately approach the heading chevron, pause on it, and show the click.
  await move(26, 157, 18);
  hold(650);
  await page.screenshot({ path: path.join(directory, 'fold-hover.png') });
  await click();
  hold(800);
  await page.screenshot({ path: path.join(directory, 'folded.png') });
  await click(); // Reopen so the final still includes the code block, too.
  await move(390, 490, 16);
  await page.evaluate(() => { document.getElementById('capture-pointer').style.display = 'none'; });
  await frame(2500);
  await fs.copyFile(path.join(directory, frames.at(-1).file), path.join(directory, 'poster.png'));

  // Verify the actual document after recording, including the drag's text edit.
  await page.keyboard.press(`${modifier}+a`);
  await page.waitForTimeout(100);
  await page.keyboard.press(`${modifier}+c`);
  await page.waitForTimeout(100);
  const markdown = await page.evaluate(() => navigator.clipboard.readText());
  await fs.writeFile(path.join(directory, 'markdown.md'), markdown);
  assert(markdown.startsWith('# todo\n'), markdown);
  assert(markdown.includes('```sh\ncargo test login\n```'), markdown);
  assert(markdown.includes('**amazon returns**'), markdown);
  assert(markdown.includes(`[shelf brackets](${bracketUrl})`), markdown);
  assert(markdown.includes('- [ ] put up shelf\n- [ ] fix doorbell'), 'Drag did not reorder the tasks:\n' + markdown);
  assert(!markdown.includes('To-do List'), markdown);
  assert(!markdown.includes('<!--'), 'The work section should be reopened in the final still.');
  assert.equal(errors.length, 0, errors.join('\n'));
  await fs.writeFile(path.join(directory, 'frames.json'), JSON.stringify({ width, height, frames }, null, 2));
  console.log(`${theme}: ${frames.length} frames, ${frames.reduce((sum, f) => sum + f.duration, 0) / 1000}s; Markdown verified.`);
  await context.close();
}

(async () => {
  const browser = await chromium.launch({ headless: true,
    ...(process.env.CHROME_EXECUTABLE ? { executablePath: process.env.CHROME_EXECUTABLE } : {}),
  });
  try {
    for (const theme of (process.env.CAPTURE_THEMES || 'light,dark').split(',')) await record(browser, theme);
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
