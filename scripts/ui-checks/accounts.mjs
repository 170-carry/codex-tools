export async function checkAccounts({ page, check, snap }) {
  const row = name => page.locator(".accountRow").filter({ hasText: name });
  const detail = page.locator(".accountDetailPanel");
  check("Saved retired-layout preference still opens the original account page",
    await page.locator(".classicAccountsWorkspace").isVisible());
  check("Layout selector and retired account table are absent",
    await page.locator(".layoutPicker,.accountTable,.accountInspector").count() === 0);
  check("Summary, filters and permanent detail panel remain visible",
    await page.locator(".metaPill").count() === 4 &&
    await page.locator(".accountFilters").isVisible() && await detail.isVisible());
  check("All account groups retain proxy switches and reset-credit counts",
    await page.locator(".rowToggle input").count() === 9 &&
    await page.locator(".accountResetCredits").count() === 9);
  check("Per-account token statistics and quota reset times remain",
    (await row("daily@example.com").locator(".accountStatsCell").innerText()).includes("42.6") &&
    await row("daily@example.com").locator(".usageMeterReset").count() === 2);
  check("Cached usage errors stay visible", (await row("cached@example.com").innerText()).includes("缓存"));
  await snap("accounts-light");
  await row("studio@example.com").click();
  check("Inspecting an account preserves the active account",
    (await detail.locator("h2").innerText()).includes("studio@example.com") &&
    (await page.locator(".accountRow.isCurrent").innerText()).includes("daily@example.com"));
  await detail.locator(".detailEditButton").click();
  await detail.locator(".detailAliasEditor input").fill("备用工作账号");
  await detail.locator(".detailAliasEditor").getByRole("button", {name:"保存", exact:true}).click();
  await page.waitForFunction(() => document.querySelector(".accountDetailPanel h2")?.textContent === "备用工作账号");
  check("Account rename updates the permanent panel", await row("备用工作账号").count() === 1);
  await row("备用工作账号").locator(".rowSwitchButton").click();
  await page.waitForFunction(() => document.querySelector(".accountRow.isCurrent")?.textContent?.includes("备用工作账号"));
  check("Switching keeps exactly one active account and records history",
    await page.locator(".accountRow.isCurrent").count() === 1 && await detail.locator(".recentSwitchList li").count() > 0);
  await row("备用工作账号").locator(".rowToggle").click();
  await page.waitForFunction(() => ![...document.querySelectorAll(".accountRow")].find(el=>el.textContent.includes("备用工作账号"))?.querySelector(".rowToggle input")?.checked);
  check("Proxy participation remains directly editable", !await row("备用工作账号").locator(".rowToggle input").isChecked());
  await detail.locator(".resetCreditItem button").first().click();
  await detail.locator(".resetCreditConfirmation").waitFor();
  await detail.locator(".resetCreditConfirmation").getByRole("button", { name: "取消", exact:true }).click();
  check("Cancelling a reset-credit confirmation consumes nothing",
    await detail.locator(".resetCreditConfirmation").count() === 0 &&
    !(await page.evaluate(() => window.__previewCommands)).includes("use_reset_credit"));
  const trigger = row("备用工作账号").locator(".rowMoreButton");
  await trigger.click();
  await page.getByRole("menu").waitFor();
  await page.keyboard.press("End");
  check("Account menu supports keyboard navigation", await page.getByRole("menuitem").last().evaluate(el=>el===document.activeElement));
  await page.keyboard.press("Escape");
  check("Closing the account menu restores focus", await trigger.evaluate(el=>el===document.activeElement));
  await detail.locator(".quickActionGrid .dangerAction").click();
  await page.getByRole("alertdialog").waitFor();
  await page.keyboard.press("Escape");
  check("Delete confirmation can be cancelled without deleting an account", await row("备用工作账号").count() === 1);
  const search = page.locator("[data-account-search]");
  await search.fill("no-match-at-all");
  check("No-match search state is explicit", await page.locator(".accountEmptyState").isVisible());
  await search.fill("");
  await page.keyboard.press(process.platform === "darwin" ? "Meta+f" : "Control+f");
  check("Find shortcut focuses the persistent account search", await search.evaluate(el=>el===document.activeElement));
}
