import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

// Use an installed Playwright package or the desktop app's bundled runtime.
const modulePath = process.env.CODEX_TOOLS_PLAYWRIGHT_MODULE;
const { chromium } = await import(
  modulePath ? pathToFileURL(modulePath).href : "playwright"
);
const output =
  process.env.CODEX_TOOLS_UI_OUTPUT_DIR ?? "/tmp/codex-tools-ui-review";
const baseUrl =
  process.env.CODEX_TOOLS_UI_BASE_URL ??
  "http://127.0.0.1:5199/tests/ui/preview.html";
await mkdir(output, { recursive: true });
const browser = await chromium.launch({
  headless: true,
  executablePath:
    process.env.CODEX_TOOLS_CHROME_PATH ??
    (process.platform === "darwin"
      ? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
      : undefined),
});
const context = await browser.newContext({
  viewport: { width: 920, height: 700 },
  deviceScaleFactor: 1,
  locale: "zh-CN",
  timezoneId: "Asia/Shanghai",
});
const page = await context.newPage();
const errors = [];
const checks = [];
context.on("page", (p) =>
  p.on("pageerror", (error) => errors.push(error.message)),
);
page.on("pageerror", (error) => errors.push(error.message));
const check = (label, condition) => {
  assert.ok(condition, label);
  checks.push(label);
  console.log(`✓ ${label}`);
};
const row = (name) => page.locator(".accountRow").filter({ hasText: name });
const nav = (name) => ({
  click: async () => {
    if (name === "设置") await page.locator(".settingsButton").click();
    else await page.getByRole("tab", { name, exact: true }).click();
  },
});
const inspector = page.locator(".accountInspector[open]");
const snap = (name) =>
  page.screenshot({ path: path.join(output, `${name}.png`) });

try {
  await page.goto(baseUrl);
  await page.locator(".accountTable").waitFor();
  check(
    "Accounts use five aligned columns",
    (await page.locator(".accountTable th").count()) === 5,
  );
  check(
    "Search is collapsed by default",
    (await page.locator("[data-account-search]").count()) === 0,
  );
  check(
    "Both full reset dates are visible",
    (await row("daily@example.com").locator("time.quotaReset").count()) === 2 &&
      /\d{4}\/\d{2}\/\d{2}/.test(
        await row("daily@example.com")
          .locator("time.quotaReset")
          .first()
          .innerText(),
      ),
  );
  check(
    "Reset credits have their own column",
    (await row("daily@example.com")
      .locator(".accountCreditsCell")
      .innerText()) === "3",
  );
  check(
    "The system time zone is identified",
    (await page.locator(".accountTimeZone").innerText()).includes(
      "Asia/Shanghai",
    ),
  );
  check(
    "All account profiles and grouped plan variants remain accessible",
    (await page.locator(".accountRow").count()) === 8,
  );
  check(
    "Current account is highlighted once with both quotas",
    (await page.locator(".accountRow.isCurrent").innerText()).includes("63%") &&
      (await page.locator(".accountRow.isCurrent").innerText()).includes("37%"),
  );
  check(
    "Cached usage failures stay visible",
    (await row("cached@example.com").innerText()).includes("缓存"),
  );
  check(
    "Unknown quotas have no fabricated progress value",
    (await row("自定义 API").locator("[aria-valuenow]").count()) === 0,
  );
  await snap("accounts-light");

  await row("studio@example.com").locator(".accountNameButton").click();
  await inspector.waitFor();
  check(
    "Inspecting another account does not change the active account",
    (await page.locator(".accountRow.isCurrent").innerText()).includes(
      "daily@example.com",
    ),
  );
  await inspector.locator(".resetCreditsToggle").click();
  check(
    "All reset credits remain available in account details",
    (await inspector.locator(".resetCreditItem").count()) === 3,
  );
  await inspector.locator(".detailEditButton").click();
  await inspector.locator(".detailAliasEditor input").fill("备用工作账号");
  await inspector.locator(".detailAliasEditor button[type=submit]").click();
  await page.waitForFunction(
    () =>
      document.querySelector("#inspector-title")?.textContent ===
      "备用工作账号",
  );
  await snap("account-details");
  await page.keyboard.press("Escape");
  await inspector.waitFor({ state: "detached" });
  check(
    "Inspector closes with Escape and restores focus",
    await row("备用工作账号")
      .locator(".accountNameButton")
      .evaluate((e) => e === document.activeElement),
  );

  const trigger = row("备用工作账号").locator(".rowMoreButton");
  await trigger.click();
  await page.getByRole("menu").waitFor();
  await page.keyboard.press("ArrowDown");
  check(
    "Account menu supports keyboard navigation",
    await page
      .getByRole("menuitem", { name: "重新登录", exact: true })
      .evaluate((e) => e === document.activeElement),
  );
  await page.keyboard.press("Escape");
  check(
    "Account menu restores keyboard focus",
    await trigger.evaluate((e) => e === document.activeElement),
  );
  await trigger.click();
  await page.getByRole("menuitemcheckbox").click();
  await trigger.click();
  check(
    "Per-account proxy toggle is preserved",
    (await page.getByRole("menuitemcheckbox").getAttribute("aria-checked")) ===
      "false",
  );
  await page.keyboard.press("Escape");

  await row("备用工作账号").locator(".rowSwitchButton").click();
  await page.waitForFunction(() =>
    document
      .querySelector(".accountRow.isCurrent")
      ?.textContent?.includes("备用工作账号"),
  );
  check(
    "Switch updates the current account and keeps a single current row",
    (await page.locator(".accountRow.isCurrent").count()) === 1,
  );
  await row("备用工作账号").locator(".accountNameButton").click();
  await inspector.locator(".inspectorHistory summary").click();
  check(
    "Successful account switches retain their history",
    (await inspector.locator(".recentSwitchList li").count()) === 1,
  );
  await inspector.locator(".inspectorFooter .rowMoreButton").click();
  check(
    "Account menus remain visible above the modal inspector",
    await page.getByRole("menu").isVisible(),
  );
  await page.getByRole("menuitem", { name: "删除账号", exact: true }).click();
  await page.locator(".deleteAccountDialog").waitFor();
  check(
    "Delete confirmation is accessible from the inspector",
    (await inspector.count()) === 0,
  );
  await page.locator(".deleteAccountDialog .ghost").click();

  await page.locator(".accountSearchButton").click();
  await page.locator(".filterButton").click();
  await page
    .locator("#account-filters select")
    .first()
    .selectOption("exhausted");
  check(
    "Status filters still work",
    (await page.locator(".accountRow").count()) === 1,
  );
  await page.locator("#account-filters button").click();
  await page.locator(".closeAccountSearch").click();
  await row("team@example.com")
    .getByRole("button", { name: "PRO", exact: true })
    .click();
  check(
    "Grouped plan variants expose their own quotas",
    (await row("team@example.com").innerText()).includes("30%"),
  );
  await page.locator(".accountsMoreButton").click();
  await page.getByRole("menuitem", { name: "全部导出", exact: true }).click();
  check(
    "Export still reaches the existing backend command",
    await page.evaluate(() =>
      window.__previewCommands.includes("export_accounts_zip"),
    ),
  );

  const checkSections = async (root, expected, prefix) => {
    const tabs = root.locator(".pageSectionTabs [role=tab]");
    check(
      `${prefix}: every section is directly accessible`,
      (await tabs.count()) === expected,
    );
    for (const tab of await tabs.all()) {
      await tab.click();
      check(
        `${prefix}: ${await tab.innerText()} opens alone`,
        (await root.locator(".pageSectionPanel:visible").count()) === 1 &&
          (await tab.getAttribute("aria-selected")) === "true",
      );
    }
    await tabs.first().click();
    await tabs.first().press("ArrowRight");
    check(
      `${prefix}: keyboard navigation moves focus with selection`,
      (await tabs.nth(1).getAttribute("aria-selected")) === "true" &&
        (await tabs.nth(1).evaluate((node) => node === document.activeElement)),
    );
    await tabs.first().click();
  };
  await nav("分析").click();
  const analyticsPage = page.locator(".analyticsPage");
  await checkSections(analyticsPage, 4, "Analytics");
  await page.locator(".tokenUsageDisclosure summary").click();
  check(
    "All four token periods remain",
    (await page.locator(".accountTokenUsageItems > span").count()) === 4,
  );
  await page.locator(".tokenUsageDisclosure summary").click();
  await analyticsPage.getByRole("tab", { name: "会话", exact: true }).click();
  await analyticsPage.locator(".analyticsSearch").fill("example");
  await analyticsPage.getByRole("tab", { name: "项目", exact: true }).click();
  await analyticsPage.getByRole("tab", { name: "会话", exact: true }).click();
  check(
    "Session search survives section switching",
    (await analyticsPage.locator(".analyticsSearch").inputValue()) ===
      "example",
  );
  await analyticsPage.locator(".analyticsSearch").fill("");
  await analyticsPage.getByRole("tab", { name: "项目", exact: true }).click();
  await snap("analytics");
  await nav("API 反代").click();
  const proxyPage = page.locator(".proxyPage");
  await checkSections(proxyPage, 5, "Proxy");
  await proxyPage.getByRole("tab", { name: "密钥与模型", exact: true }).click();
  await proxyPage.locator(".proxyKeyCreateRow input").first().fill("Draft key");
  await proxyPage.getByRole("tab", { name: "服务", exact: true }).click();
  await proxyPage.getByRole("tab", { name: "密钥与模型", exact: true }).click();
  check(
    "Unsubmitted key label survives section switching",
    (await proxyPage
      .locator(".proxyKeyCreateRow input")
      .first()
      .inputValue()) === "Draft key",
  );
  await proxyPage.locator(".proxySubmenuTrigger").click();
  await page.locator(".proxyModelDialog").waitFor();
  await page.keyboard.press("Escape");
  check(
    "Model dialog closes and returns focus",
    (await page.locator(".proxyModelDialog").count()) === 0 &&
      (await proxyPage
        .locator(".proxySubmenuTrigger")
        .evaluate((node) => node === document.activeElement)),
  );
  await proxyPage.getByRole("tab", { name: "服务", exact: true }).click();
  await snap("proxy");
  await nav("设置").click();
  await checkSections(page.locator(".settingsPage"), 5, "Settings");
  await snap("settings");
  await nav("账号").click();
  await nav("设置").click();
  await page.locator(".settingsPage .themeSwitch").first().click();
  await nav("账号").click();
  await page.waitForFunction(
    () => document.documentElement.dataset.theme === "dark",
  );
  await snap("accounts-dark");
  check(
    "Dark appearance is available",
    (await page.locator("html").getAttribute("data-theme")) === "dark",
  );
  await nav("设置").click();
  await page.locator(".settingsPage .themeSwitch").first().click();
  await nav("账号").click();

  const modifier = await page.evaluate(() =>
    /Mac/i.test(navigator.platform) ? "Meta" : "Control",
  );
  await page.keyboard.press(`${modifier}+f`);
  check(
    "Find shortcut focuses account search",
    await page
      .locator("[data-account-search]")
      .evaluate((e) => e === document.activeElement),
  );
  await page
    .locator("[data-account-search]")
    .fill("no-account-matches-this-query");
  check(
    "No-match state is explicit",
    await page.locator(".accountEmptyState").isVisible(),
  );
  await page.locator("[data-account-search]").fill("");
  await page.keyboard.press(`${modifier}+,`);
  await page.locator(".settingsPage").waitFor();
  check(
    "Settings shortcut works",
    await page.locator(".settingsPage").isVisible(),
  );
  await page.keyboard.press(`${modifier}+1`);
  await page.locator(".accountTable").waitFor();
  await page.setViewportSize({ width: 640, height: 500 });
  await snap("accounts-small-window");
  check(
    "Minimum desktop window has no horizontal page overflow",
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  );
  check(
    "Account summary remains visible in a small window",
    await page
      .locator(".accountListFoot")
      .evaluate((e) => e.getBoundingClientRect().bottom <= innerHeight),
  );
  await page.setViewportSize({ width: 920, height: 700 });

  for (const scenario of ["empty", "no-active", "unknown"]) {
    await page.goto(`${baseUrl}?scenario=${scenario}`);
    await page
      .locator(scenario === "empty" ? ".accountEmptyState" : ".accountTable")
      .waitFor();
    check(
      `Account scenario renders: ${scenario}`,
      scenario === "unknown"
        ? (await page.locator(".accountTable [aria-valuenow]").count()) === 0
        : (await page.locator(".accountRow.isCurrent").count()) === 0,
    );
  }
  await page.goto(`${baseUrl}?locale=en-US`);
  await page.locator(".accountTable").waitFor();
  check(
    "English interface is retained",
    (await page.locator(".accountRow.isCurrent").innerText()).includes(
      "In use",
    ),
  );
  check("No browser runtime errors", errors.length === 0);
  await writeFile(
    path.join(output, "ui-checks.json"),
    JSON.stringify({ checks, errors }, null, 2),
  );
} finally {
  await browser.close();
}
