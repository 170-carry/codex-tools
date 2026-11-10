import assert from "node:assert/strict";
import test from "node:test";
import { persistLocaleWhenReady } from "../src/utils/localePersistence.ts";
import { createProjectCommandGate } from "../src/utils/localProjectSwitch.ts";

test("locale startup waits for a successful settings read, even before recovery is known", async () => {
  const patches: unknown[] = [];
  const persist = async (patch: unknown) => { patches.push(patch); };
  assert.equal(await persistLocaleWhenReady({
    locale: "en-US", persistedLocale: "zh-CN", settingsLoaded: false, recoveryRequired: false,
  }, persist), false);
  assert.deepEqual(patches, []);
  assert.equal(await persistLocaleWhenReady({
    locale: "en-US", persistedLocale: "zh-CN", settingsLoaded: true, recoveryRequired: false,
  }, persist), true);
  assert.deepEqual(patches, [{ locale: "en-US" }]);
});

test("locale changes remain deferred throughout recovery and persist after successful reload", async () => {
  const patches: unknown[] = [];
  const persist = async (patch: unknown) => { patches.push(patch); };
  for (const settingsLoaded of [false, true]) {
    assert.equal(await persistLocaleWhenReady({
      locale: "ja-JP", persistedLocale: "zh-CN", settingsLoaded, recoveryRequired: true,
    }, persist), false);
  }
  assert.deepEqual(patches, []);
  assert.equal(await persistLocaleWhenReady({
    locale: "ko-KR", persistedLocale: "zh-CN", settingsLoaded: true, recoveryRequired: false,
  }, persist), true);
  assert.deepEqual(patches, [{ locale: "ko-KR" }]);
});

test("normal subsequent language changes persist without repeating an already saved value", async () => {
  const patches: unknown[] = [];
  const persist = async (patch: unknown) => { patches.push(patch); };
  assert.equal(await persistLocaleWhenReady({
    locale: "en-US", persistedLocale: "en-US", settingsLoaded: true, recoveryRequired: false,
  }, persist), false);
  assert.equal(await persistLocaleWhenReady({
    locale: "ru-RU", persistedLocale: "en-US", settingsLoaded: true, recoveryRequired: false,
  }, persist), true);
  assert.deepEqual(patches, [{ locale: "ru-RU" }]);
});

test("the unified gate catches recovery detected after the locale effect captured its readiness", async () => {
  const calls: string[] = [];
  const gate = createProjectCommandGate(async <T>(command: string) => {
    calls.push(command);
    return undefined as T;
  }, () => {});
  const captured = { locale: "en-US", persistedLocale: "zh-CN", settingsLoaded: true, recoveryRequired: false } as const;
  gate.requireRecovery();
  await assert.rejects(persistLocaleWhenReady(captured, (patch) => gate.invoke("update_app_settings", { patch })), /RECOVERY_REQUIRED/);
  assert.deepEqual(calls, []);
});
