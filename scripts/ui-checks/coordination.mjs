import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";

const { chromium } = await import(process.env.CODEX_TOOLS_PLAYWRIGHT_MODULE
  ? pathToFileURL(process.env.CODEX_TOOLS_PLAYWRIGHT_MODULE).href : "playwright");
const output = process.env.CODEX_TOOLS_UI_OUTPUT_DIR ?? ".codex-verification/coordination";
const base = process.env.CODEX_TOOLS_UI_BASE_URL ?? "http://127.0.0.1:5199/tests/ui/preview.html";
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ headless: true, executablePath: process.env.CODEX_TOOLS_CHROME_PATH ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" });
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const checks = [], errors = [];
page.on("pageerror", e => errors.push(e.message));
const check = (label, condition) => { assert.ok(condition, label); checks.push(label); };
const snap = name => page.screenshot({ path: `${output}/${name}.png`, animations: "disabled" });
const noPageOverflow = () => page.evaluate(() => document.documentElement.scrollWidth <= innerWidth);
try {
  await page.goto(`${base}?accounts=4`);
  await page.locator(".accountNameButton").first().click();
  for (const width of [1920, 1600, 1440, 1377, 1376, 1375, 1280, 1024, 768, 600, 480]) {
    await page.setViewportSize({ width, height: 900 });
    await page.waitForTimeout(220);
    const geometry = await page.evaluate(() => {
      const list = document.querySelector(".accountListPanel").getBoundingClientRect();
      const panel = document.querySelector(".accountInspector");
      const detail = panel.getBoundingClientRect();
      return { aligned: Math.abs(list.top - detail.top) < 2, docked: panel.getAttribute("role") === "region", row: document.querySelector(".accountRow").getBoundingClientRect().height, overlap: list.right > detail.left };
    });
    check(`Accounts ${width}: no page overflow and aligned panels`, await noPageOverflow() && geometry.aligned);
    check(`Accounts ${width}: docked panels never overlap`, !geometry.docked || !geometry.overlap);
    if (width >= 1024) check(`Accounts ${width}: compact row density`, geometry.row <= 80);
    if ([1440, 1280, 600].includes(width)) await snap(`accounts-${width}`);
  }
  for (let i = 0; i < 24; i++) {
    await page.keyboard.press("Tab");
    check(`Inspector narrow focus ${i}`, await page.evaluate(() => !!document.activeElement?.closest(".accountInspector")));
  }
  await page.keyboard.press("Escape");
  await page.locator(".accountInspector").waitFor({ state: "detached" });
  check("Inspector restores account trigger", await page.locator(".accountNameButton").first().evaluate(e => e === document.activeElement));
  await page.setViewportSize({ width: 600, height: 900 });
  const search = page.locator("[data-account-search]");
  await search.fill("no match"); await search.press("Escape");
  check("Search Escape clears and retains focus", await search.inputValue() === "" && await search.evaluate(e => e === document.activeElement));
  for (const width of [1440, 1024, 600, 480]) {
    await page.setViewportSize({ width, height: 900 });
    await page.locator(".sidebarItem.isSettings").click();
    await page.locator(".trayIconStyleOption").first().waitFor();
    check(`Settings ${width}: readable previews`, await page.locator(".trayIconStyleOption").evaluateAll(es => es.length === 6 && es.every(e => e.clientWidth >= 98 && e.scrollWidth <= e.clientWidth)));
    check(`Settings ${width}: no page overflow`, await noPageOverflow());
    await page.locator(".trayIconStyleOption").first().scrollIntoViewIfNeeded();
    await snap(`settings-${width}`);
    await page.getByRole("button", { name: "分析", exact: true }).click();
    await page.locator(".analyticsOverview").waitFor();
    check(`Analytics ${width}: no page overflow`, await noPageOverflow());
    check(`Search ${width}: inner input stays inside border`, await page.locator(".analyticsPage .uiSearchField").evaluate(e => { const outer = e.getBoundingClientRect(), inner = e.querySelector("input").getBoundingClientRect(); return inner.top > outer.top && inner.bottom < outer.bottom; }));
    await page.getByRole("button", { name: "估算费用", exact: true }).click();
    check(`Analytics ${width}: explicit metric selection`, await page.getByRole("button", { name: "估算费用", exact: true }).getAttribute("aria-pressed") === "true");
    await snap(`analytics-${width}`);
    await page.getByRole("button", { name: "API 反代", exact: true }).click();
    for (const tab of ["用量", "配置", "远程服务器", "公网访问"]) {
      await page.getByRole("tab", { name: tab, exact: true }).click();
      check(`Proxy ${width} ${tab}: no page overflow`, await noPageOverflow());
      if (tab === "远程服务器") check(`Remote ${width}: no unrelated local header`, !await page.locator(".proxyLocalControlCard").isVisible());
      await snap(`proxy-${tab}-${width}`);
    }
  }
  await page.getByRole("button", { name: "Claude", exact: true }).click();
  await page.locator(".claudeAccountsView").waitFor();
  check("Claude narrow: no page overflow", await noPageOverflow());
  await page.getByRole("button", { name: /添加账号/ }).click();
  await page.locator(".claudeImportDialog").waitFor();
  check("Claude authorization: no page overflow", await noPageOverflow());
  await snap("claude-authorization-narrow");
  await page.keyboard.press("Escape");
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.locator(".sidebarItem.isSettings").click();
  check("Reduced motion disables page animations", await page.locator(".uiPage").evaluate(e => getComputedStyle(e).animationName === "none"));
  await page.locator(".topActions button").first().click();
  await snap("settings-narrow-dark");
  for (const locale of ["en-US", "ja-JP", "ko-KR", "ru-RU"]) {
    await page.goto(`${base}?locale=${locale}`);
    await page.locator(".sidebarItem.isSettings").click();
    await page.locator(".trayIconStyleOption").first().waitFor();
    check(`Settings ${locale}: no overflow`, await noPageOverflow() && await page.locator(".trayIconStyleOption").evaluateAll(es => es.every(e => e.scrollWidth <= e.clientWidth)));
  }
  check("No browser runtime errors", errors.length === 0);
} finally {
  await writeFile(`${output}/verification.json`, JSON.stringify({ checks, errors }, null, 2));
  await browser.close();
}
console.log(`${checks.length} browser checks passed`);
