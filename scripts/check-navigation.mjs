import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import path from "node:path";

const modulePath = process.env.CODEX_TOOLS_PLAYWRIGHT_MODULE;
const { chromium } = await import(modulePath ? pathToFileURL(modulePath).href : "playwright");
const output = process.env.CODEX_TOOLS_UI_OUTPUT_DIR ?? "/tmp/codex-tools-issue219/navigation";
const url = process.env.CODEX_TOOLS_UI_BASE_URL ?? "http://127.0.0.1:5199/tests/ui/preview.html";
await mkdir(output, { recursive: true });
const browser = await chromium.launch({
  headless: true,
  executablePath: process.env.CODEX_TOOLS_CHROME_PATH ??
    (process.platform === "darwin" ? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" : undefined),
});
const checks = [];
const errors = [];
const check = (label, condition) => {
  assert.ok(condition, label);
  checks.push(label);
  console.log(`✓ ${label}`);
};

try {
  for (const layout of ["classic", "compact"]) {
    const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, locale: "zh-CN" });
    await context.addInitScript((layout) => localStorage.setItem("codex-tools-layout", layout), layout);
    const page = await context.newPage();
    page.on("pageerror", (error) => errors.push(error.message));
    let releaseModule;
    const moduleGate = new Promise((resolve) => { releaseModule = resolve; });
    let moduleRequested;
    const moduleStarted = new Promise((resolve) => { moduleRequested = resolve; });
    await page.route("**/src/components/workspace/ProxyView.tsx*", async (route) => {
      moduleRequested();
      await moduleGate;
      await route.continue();
    });
    await page.goto(`${url}?proxy=bound`);
    await page.locator(layout === "classic" ? ".classicAccountsWorkspace" : ".accountTable").waitFor();
    const proxyTab = layout === "classic"
      ? page.locator(".topSegmentedButton").nth(2)
      : page.locator(".viewTab").nth(2);
    await proxyTab.hover();
    await moduleStarted;
    check(`${layout}: approaching a tab preloads its view`, true);
    await proxyTab.click();
    check(`${layout}: a delayed view keeps the current page visible`,
      await page.locator(".accountsPage").isVisible());
    check(`${layout}: no blank loading replacement during navigation`,
      !(await page.locator(".workspaceLoading").isVisible()));
    releaseModule();
    await page.locator(".proxyPage").waitFor();
    check(`${layout}: navigation completes once the chunk is ready`, await page.locator(".proxyPage").isVisible());
    const sync = page.getByRole("button", { name: "同步模型目录", exact: true });
    check(`${layout}: an existing proxy binding can sync its catalog`, await sync.isEnabled());
    await sync.click();
    await page.waitForFunction(() => window.__previewCommands.includes("bind_codex_to_api_proxy"));
    check(`${layout}: catalog sync tells users to restart Codex`,
      (await page.locator("body").innerText()).includes("请重启 Codex"));
    await page.screenshot({ path: path.join(output, `${layout}-proxy.png`) });
    if (layout === "classic") {
      await page.locator(".topSegmentedButton").nth(0).click();
      await page.locator(".accountsPage").waitFor();
      await page.waitForFunction(() => {
        const indicator = document.querySelector(".topSegmentedIndicator").getBoundingClientRect();
        const active = document.querySelector(".topSegmentedButton.isActive").getBoundingClientRect();
        return Math.abs(indicator.x - active.x) < 1 && Math.abs(indicator.width - active.width) < 1;
      });
      check("Classic navigation indicator aligns with the active button", true);
      await page.setViewportSize({ width: 760, height: 740 });
      check("Classic navigation fits a narrow window",
        await page.locator(".topSegmentedNav").evaluate((element) => element.scrollWidth <= element.clientWidth));
      await page.emulateMedia({ reducedMotion: "reduce" });
      check("Reduced-motion preference disables the navigation transition",
        await page.locator(".topSegmentedIndicator").evaluate((element) =>
          Number.parseFloat(getComputedStyle(element).transitionDuration) < .001));
      await page.screenshot({ path: path.join(output, "classic-accounts-narrow.png") });
    }
    await context.close();
  }
  check("No browser runtime errors", errors.length === 0);
  await writeFile(path.join(output, "results.json"), JSON.stringify({ checks, errors }, null, 2));
} finally {
  await browser.close();
}
