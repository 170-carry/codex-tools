import assert from "node:assert/strict";
import test from "node:test";
import {
  applyLiveQuotaDisplayUpdate,
  canDisableQuotaDisplay,
  hasActiveQuotaDisplay,
} from "../src/utils/quotaDisplayOnboarding.ts";

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
