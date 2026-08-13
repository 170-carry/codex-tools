import assert from "node:assert/strict";
import test from "node:test";
import {
  applyLiveQuotaDisplayUpdate,
  buildMacosQuotaOnboardingPatch,
  canDisableQuotaDisplay,
  hasActiveQuotaDisplay,
  shouldOpenQuotaOnboarding,
} from "../src/utils/quotaDisplayOnboarding.ts";

test("macOS confirmation reapplies the complete selected configuration", () => {
  assert.deepEqual(
    buildMacosQuotaOnboardingPatch({
      statusBarEnabled: true,
      statusBarMode: "remaining",
      trayEnabled: false,
      trayIconStyle: "logoProgressRing",
      showLogoRingPercentage: false,
    }),
    {
      trayUsageDisplayMode: "remaining",
      windowsTrayIconStyle: "logoProgressRing",
      trayQuotaIconVisible: false,
      macosTrayLogoRingShowPercentage: false,
      macosQuotaOnboardingCompleted: true,
    },
  );
});

test("macOS confirmation persists hidden status text when only the quota icon is enabled", () => {
  const patch = buildMacosQuotaOnboardingPatch({
    statusBarEnabled: false,
    statusBarMode: "oneWeekRemaining",
    trayEnabled: true,
    trayIconStyle: "gradientNumberPlate",
    showLogoRingPercentage: true,
  });

  assert.equal(patch.trayUsageDisplayMode, "hidden");
  assert.equal(patch.trayQuotaIconVisible, true);
  assert.equal(patch.macosQuotaOnboardingCompleted, true);
});

test("macOS confirmation allows both quota displays to remain disabled", () => {
  const patch = buildMacosQuotaOnboardingPatch({
    statusBarEnabled: false,
    statusBarMode: "oneWeekRemaining",
    trayEnabled: false,
    trayIconStyle: "gradientNumber",
    showLogoRingPercentage: false,
  });

  assert.equal(patch.trayUsageDisplayMode, "hidden");
  assert.equal(patch.trayQuotaIconVisible, false);
  assert.equal(patch.macosQuotaOnboardingCompleted, true);
});

test("at least one quota display remains enabled", () => {
  assert.equal(hasActiveQuotaDisplay(true, false), true);
  assert.equal(hasActiveQuotaDisplay(false, true), true);
  assert.equal(hasActiveQuotaDisplay(false, false), false);
  assert.equal(canDisableQuotaDisplay(true), true);
  assert.equal(canDisableQuotaDisplay(false), false);
});

test("successful live updates keep the optimistic selection", async () => {
  let enabled = false;
  const applied = await applyLiveQuotaDisplayUpdate({
    patch: { enabled: true },
    applyLocal: () => {
      enabled = true;
    },
    rollbackLocal: () => {
      enabled = false;
    },
    persist: async () => undefined,
  });

  assert.equal(applied, true);
  assert.equal(enabled, true);
});

test("failed live updates restore the previous selection", async () => {
  let enabled = false;
  const applied = await applyLiveQuotaDisplayUpdate({
    patch: { enabled: true },
    applyLocal: () => {
      enabled = true;
    },
    rollbackLocal: () => {
      enabled = false;
    },
    persist: async () => {
      throw new Error("native surface update failed");
    },
  });

  assert.equal(applied, false);
  assert.equal(enabled, false);
});

test("onboarding completion is tracked independently for Windows and macOS", () => {
  assert.equal(
    shouldOpenQuotaOnboarding({
      platform: "windows",
      settingsLoaded: true,
      windowsCompleted: false,
      macosCompleted: true,
    }),
    true,
  );
  assert.equal(
    shouldOpenQuotaOnboarding({
      platform: "macos",
      settingsLoaded: true,
      windowsCompleted: true,
      macosCompleted: false,
    }),
    true,
  );
  assert.equal(
    shouldOpenQuotaOnboarding({
      platform: "macos",
      settingsLoaded: false,
      windowsCompleted: false,
      macosCompleted: false,
    }),
    false,
  );
  assert.equal(
    shouldOpenQuotaOnboarding({
      platform: null,
      settingsLoaded: true,
      windowsCompleted: false,
      macosCompleted: false,
    }),
    false,
  );
});
