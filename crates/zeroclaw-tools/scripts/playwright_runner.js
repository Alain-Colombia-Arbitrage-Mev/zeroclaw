#!/usr/bin/env node
// Playwright playbook runner — spawned by zeroclaw-tools/playwright.rs.
//
// Reads a JSON playbook from stdin:
//   {
//     "url": "https://example.com",
//     "headless": true,                 // optional, default true
//     "timeoutMs": 30000,               // optional, default 30 s
//     "viewport": { "width": 1280, "height": 720 },  // optional
//     "userAgent": "...",               // optional
//     "steps": [
//       { "action": "click",         "selector": "button#login" },
//       { "action": "fill",          "selector": "#user", "value": "alice" },
//       { "action": "wait",          "selector": ".dashboard", "timeout": 5000 },
//       { "action": "extract_text",  "selector": "h1" },
//       { "action": "extract_html",  "selector": "main" },
//       { "action": "screenshot",    "fullPage": true },
//       { "action": "evaluate",      "expression": "document.title" },
//       { "action": "press_key",     "key": "Enter" },
//       { "action": "navigate",      "url": "https://other.example" }
//     ]
//   }
//
// Writes a single JSON object to stdout:
//   {
//     "success": true,
//     "results": [ { "action": "...", "ok": true, "value": ... }, ... ],
//     "finalUrl": "..."
//   }

'use strict';

const { chromium } = require('playwright');

async function readStdin() {
  return new Promise((resolve, reject) => {
    let buf = '';
    process.stdin.setEncoding('utf8');
    process.stdin.on('data', (chunk) => { buf += chunk; });
    process.stdin.on('end', () => resolve(buf));
    process.stdin.on('error', reject);
  });
}

async function runStep(page, step) {
  const action = step.action;
  switch (action) {
    case 'navigate': {
      const r = await page.goto(step.url, { waitUntil: step.waitUntil || 'load' });
      return { ok: true, status: r ? r.status() : null, url: page.url() };
    }
    case 'click': {
      await page.click(step.selector, { timeout: step.timeout || 10000 });
      return { ok: true };
    }
    case 'fill': {
      await page.fill(step.selector, String(step.value ?? ''), {
        timeout: step.timeout || 10000,
      });
      return { ok: true };
    }
    case 'press_key': {
      await page.keyboard.press(step.key);
      return { ok: true };
    }
    case 'wait': {
      if (step.selector) {
        await page.waitForSelector(step.selector, {
          timeout: step.timeout || 10000,
          state: step.state || 'visible',
        });
      } else if (step.url) {
        await page.waitForURL(step.url, { timeout: step.timeout || 10000 });
      } else if (step.ms) {
        await page.waitForTimeout(step.ms);
      } else {
        await page.waitForLoadState('networkidle', { timeout: step.timeout || 10000 });
      }
      return { ok: true };
    }
    case 'extract_text': {
      if (step.selector) {
        const el = await page.locator(step.selector).first();
        return { ok: true, value: await el.innerText({ timeout: step.timeout || 10000 }) };
      }
      return { ok: true, value: await page.innerText('body') };
    }
    case 'extract_html': {
      if (step.selector) {
        const el = await page.locator(step.selector).first();
        return { ok: true, value: await el.innerHTML({ timeout: step.timeout || 10000 }) };
      }
      return { ok: true, value: await page.content() };
    }
    case 'screenshot': {
      const buf = await page.screenshot({ fullPage: !!step.fullPage });
      return { ok: true, value: buf.toString('base64') };
    }
    case 'evaluate': {
      const v = await page.evaluate(step.expression);
      return { ok: true, value: v };
    }
    default:
      return { ok: false, error: `Unknown action: ${action}` };
  }
}

(async () => {
  let playbook;
  try {
    const raw = await readStdin();
    playbook = JSON.parse(raw);
  } catch (err) {
    process.stdout.write(JSON.stringify({
      success: false,
      error: `Failed to parse playbook from stdin: ${err.message}`,
    }));
    process.exit(2);
  }

  const headless = playbook.headless !== false;
  const totalTimeout = Number(playbook.timeoutMs) || 30000;

  let browser;
  try {
    browser = await chromium.launch({ headless });
    const ctxOpts = {};
    if (playbook.viewport) ctxOpts.viewport = playbook.viewport;
    if (playbook.userAgent) ctxOpts.userAgent = playbook.userAgent;
    const ctx = await browser.newContext(ctxOpts);
    const page = await ctx.newPage();
    page.setDefaultTimeout(totalTimeout);

    const results = [];
    if (playbook.url) {
      const r = await page.goto(playbook.url, { waitUntil: 'load' });
      results.push({
        action: 'navigate',
        ok: true,
        status: r ? r.status() : null,
        url: page.url(),
      });
    }

    for (const step of playbook.steps || []) {
      try {
        const out = await runStep(page, step);
        results.push({ action: step.action, ...out });
      } catch (err) {
        results.push({ action: step.action, ok: false, error: err.message });
        break; // stop on first failure
      }
    }

    process.stdout.write(JSON.stringify({
      success: results.every((r) => r.ok),
      results,
      finalUrl: page.url(),
    }));
  } catch (err) {
    process.stdout.write(JSON.stringify({
      success: false,
      error: `Playwright runner failed: ${err.message}`,
    }));
    process.exit(1);
  } finally {
    if (browser) await browser.close().catch(() => undefined);
  }
})();
