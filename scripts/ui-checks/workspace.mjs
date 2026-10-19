export async function checkWorkspace({ page, check, snap, baseUrl }) {
  const navigate = async (index, selector) => {
    await page.locator(".topSegmentedButton").nth(index).click();
    await page.locator(selector).waitFor();
  };
  await navigate(1, ".analyticsPage");
  check("Analytics keeps all four sections and editable budget visible",
    await page.locator(".classicAnalyticsSection").count() === 4 &&
    await page.locator(".analyticsBudget input").isVisible() &&
    await page.locator('[role="tabpanel"]').count() === 0);
  await page.locator(".analyticsSearch").fill("no-session-match");
  check("Session search remains functional", await page.locator(".analyticsBlockSessions .analyticsNoResults").isVisible());
  await snap("analytics");
  await navigate(2, ".proxyPage");
  check("Proxy service, keys, remote and public controls remain available",
    await page.locator(".proxyPortInput").count() === 1 &&
    await page.locator(".proxyKeyManagerCard").count() === 1 &&
    await page.locator(".remoteSectionHeading").count() > 0);
  check("Proxy no longer uses tab panels", await page.locator('[role="tabpanel"]').count() === 0);
  await snap("proxy");
  await navigate(3, ".settingsPage");
  check("All five settings groups remain mounted without layout selection",
    await page.locator(".classicSettingsGroup").count() === 5 &&
    await page.locator(".layoutPicker").count() === 0);
  await page.locator(".topActions button").first().click();
  check("Theme toggle retains dark appearance", await page.locator("html").getAttribute("data-theme") === "dark");
  await snap("settings-dark");
  await navigate(0, ".accountsPage");
  await page.setViewportSize({width:640,height:500});
  check("Minimum window has no horizontal page overflow",
    await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth));
  await snap("accounts-small-window");
  for (const scenario of ["empty","no-active","unknown"]) {
    await page.goto(`${baseUrl}?scenario=${scenario}`);
    await page.locator(".classicAccountsWorkspace").waitFor();
    check(`Account scenario renders: ${scenario}`,
      scenario === "empty" ? await page.locator(".accountEmptyState").isVisible() : await page.locator(".accountRow").count() === 9);
  }
  await page.goto(`${baseUrl}?locale=en-US`);
  await page.locator(".classicAccountsWorkspace").waitFor();
  check("English interface remains available", await page.getByRole("button",{name:"Accounts",exact:true}).count() === 1);
  await page.reload();
  await page.locator(".classicAccountsWorkspace").waitFor();
  check("Reload cannot restore the retired account table", await page.locator(".accountTable,.layoutPicker").count() === 0);
}
