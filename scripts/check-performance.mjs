// Controlled frontend benchmark: synthetic accounts and mocked native commands.
// Alternate cached/uncached Intl on the same build to reduce run-order noise.
import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

const modulePath = process.env.CODEX_TOOLS_PLAYWRIGHT_MODULE;
const { chromium } = await import(modulePath ? pathToFileURL(modulePath).href : "playwright");
const baseUrl = process.env.CODEX_TOOLS_UI_BASE_URL ?? "http://127.0.0.1:5199/tests/ui/preview.html";
const output = process.env.CODEX_TOOLS_UI_OUTPUT_DIR ?? "/tmp/codex-tools-performance";
await mkdir(output, { recursive: true });
const browser = await chromium.launch({
  headless: true,
  executablePath: process.env.CODEX_TOOLS_CHROME_PATH ??
    (process.platform === "darwin" ? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" : undefined),
});
const results = {};
const median = (values) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];

async function filterRows(page, value, expected) {
  return page.evaluate(({ value, expected }) => new Promise((resolve) => {
    const input = document.querySelector("[data-account-search]");
    const start = performance.now();
    const observer = new MutationObserver(() => {
      if (document.querySelectorAll(".accountRow").length === expected) {
        observer.disconnect();
        resolve(performance.now() - start);
      }
    });
    observer.observe(document.querySelector(".accountsPage"), { subtree: true, childList: true });
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set.call(input, value);
    input.dispatchEvent(new Event("input", { bubbles: true }));
  }), { value, expected });
}

try {
  for (const layout of ["classic", "compact"]) {
    const samples = { cached: [], uncached: [] };
    for (let run = 0; run < 10; run++) {
      const cached = run % 2 === 0;
      const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale: "zh-CN" });
      await context.addInitScript((layout) => localStorage.setItem("codex-tools-layout", layout), layout);
      const page = await context.newPage();
      if (!cached) {
        await page.route("**/src/utils/intlFormatters.ts*", (route) => route.fulfill({
          contentType: "text/javascript",
          body: "export const dateFormatter=(...args)=>new Intl.DateTimeFormat(...args);" +
            "export const numberFormatter=(...args)=>new Intl.NumberFormat(...args);" +
            "export const getSystemTimeZone=()=>Intl.DateTimeFormat().resolvedOptions().timeZone;",
        }));
      }
      await page.goto(`${baseUrl}?accounts=100`);
      await page.waitForFunction(() => document.querySelectorAll(".accountRow").length === 100);
      await page.waitForTimeout(500); // Let the initial row animation finish in both modes.
      const commands = await page.evaluate(() => window.__previewCommands.reduce(
        (counts, command) => ({ ...counts, [command]: (counts[command] ?? 0) + 1 }), {},
      ));
      for (const command of ["list_accounts", "get_api_proxy_status", "get_cloudflared_status", "get_codex_token_usage"]) {
        assert.equal(commands[command], 1, `${layout}: ${command} must run once during startup`);
      }
      if (layout === "compact") await page.locator(".accountSearchButton").click();
      await filterRows(page, "synthetic-000", 1);
      samples[cached ? "cached" : "uncached"].push(await filterRows(page, "", 100));
      if (run === 0) {
        if (layout === "classic") await page.locator(".topSegmentedButton").nth(3).click();
        else await page.locator(".settingsButton").click();
        await page.locator(".settingsPage").waitFor();
        const maxOuterScroll = await page.evaluate(() => {
          let max = 0;
          for (const target of document.querySelectorAll(".settingsPage input,.settingsPage select,.settingsPage button")) {
            if (target.disabled || !target.getClientRects().length) continue;
            target.focus();
            for (const node of document.querySelectorAll("html,body,#root,.shell,.panel")) max = Math.max(max, Math.abs(node.scrollTop));
          }
          return max;
        });
        assert.equal(maxOuterScroll, 0, `${layout}: focusing settings must not scroll the shell`);
        results[layout] = { startup_commands: commands, max_outer_scroll: maxOuterScroll };
      }
      await context.close();
    }
    Object.assign(results[layout], {
      restore_100_rows_ms: samples,
      median_ms: { cached: median(samples.cached), uncached: median(samples.uncached) },
    });
    console.log(layout, results[layout]);
  }
  await writeFile(path.join(output, "performance.json"), JSON.stringify(results, null, 2) + "\n");
} finally {
  await browser.close();
}
