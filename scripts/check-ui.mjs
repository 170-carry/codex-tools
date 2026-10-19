import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { checkAccounts } from "./ui-checks/accounts.mjs";
import { checkWorkspace } from "./ui-checks/workspace.mjs";

const modulePath = process.env.CODEX_TOOLS_PLAYWRIGHT_MODULE;
const { chromium } = await import(modulePath ? pathToFileURL(modulePath).href : "playwright");
const output = process.env.CODEX_TOOLS_UI_OUTPUT_DIR ?? "/tmp/codex-tools-ui-review";
const baseUrl = process.env.CODEX_TOOLS_UI_BASE_URL ?? "http://127.0.0.1:5199/tests/ui/preview.html";
await mkdir(output, { recursive: true });
const browser = await chromium.launch({
  headless: true,
  executablePath: process.env.CODEX_TOOLS_CHROME_PATH ??
    (process.platform === "darwin" ? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" : undefined),
});
const context = await browser.newContext({
  viewport: { width: 1586, height: 992 }, locale: "zh-CN", timezoneId: "Asia/Shanghai",
});
// An old saved preference must never revive the retired presentation.
await context.addInitScript(() => localStorage.setItem("codex-tools-layout", "compact"));
const page = await context.newPage();
const checks = [];
const errors = [];
page.on("pageerror", error => errors.push(error.message));
const check = (label, condition) => {
  assert.ok(condition, label);
  checks.push(label);
  console.log(`✓ ${label}`);
};
const snap = name => page.screenshot({ path: path.join(output, `${name}.png`), animations: "disabled" });
try {
  await page.goto(baseUrl);
  await page.locator(".classicAccountsWorkspace").waitFor();
  await checkAccounts({ page, check, snap });
  await checkWorkspace({ page, check, snap, baseUrl });
  check("No browser runtime errors", errors.length === 0);
  await writeFile(path.join(output, "ui-checks.json"), JSON.stringify({ checks, errors }, null, 2));
} finally {
  await browser.close();
}
