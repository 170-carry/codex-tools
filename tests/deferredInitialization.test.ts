import assert from "node:assert/strict";
import test from "node:test";
import { createDeferredInitialization } from "../src/utils/deferredInitialization.ts";
import { createProjectCommandGate } from "../src/utils/localProjectSwitch.ts";

test("startup recovery defers proxy initialization until reload succeeds, then loads models and keys", async () => {
  const calls: string[] = [];
  let models: string[] = [];
  let keys: string[] = [];
  let keysLoading = true;
  const gate = createProjectCommandGate(async <T>(command: string) => {
    calls.push(command);
    return (command === "get_api_proxy_supported_models" ? ["synthetic-model-a", "synthetic-model-b"] : ["synthetic-key"]) as T;
  }, () => {});
  const tasks = [
    async () => { models = await gate.invoke<string[]>("get_api_proxy_supported_models"); },
    async () => {
      try { keys = await gate.invoke<string[]>("list_api_proxy_keys"); }
      finally { keysLoading = false; }
    },
  ];
  const initialize = createDeferredInitialization();
  await initialize({ settingsLoaded: false, recoveryRequired: false }, tasks);
  gate.requireRecovery();
  await initialize({ settingsLoaded: false, recoveryRequired: true }, tasks);
  gate.allowRecoveryReload();
  await initialize({ settingsLoaded: true, recoveryRequired: true }, tasks);
  assert.deepEqual(calls, []);
  gate.finishRecovery();
  assert.equal(await initialize({ settingsLoaded: true, recoveryRequired: false }, tasks), true);
  assert.deepEqual(models, ["synthetic-model-a", "synthetic-model-b"]);
  assert.deepEqual(keys, ["synthetic-key"]);
  assert.equal(keysLoading, false);
  await initialize({ settingsLoaded: true, recoveryRequired: false }, tasks);
  assert.deepEqual(calls, ["get_api_proxy_supported_models", "list_api_proxy_keys"]);
});

test("normal initialization runs once and recovery allows one new initialization", async () => {
  const initialize = createDeferredInitialization();
  let count = 0;
  const tasks = [async () => { count++; }];
  const ready = { settingsLoaded: true, recoveryRequired: false };
  assert.equal(await initialize(ready, tasks), true);
  assert.equal(await initialize(ready, tasks), false);
  await initialize({ ...ready, recoveryRequired: true }, tasks);
  assert.equal(await initialize(ready, tasks), true);
  assert.equal(await initialize(ready, tasks), false);
  assert.equal(count, 2);
});

test("rerenders do not duplicate an in-flight initialization and one failure does not skip other tasks", async () => {
  const initialize = createDeferredInitialization();
  const ready = { settingsLoaded: true, recoveryRequired: false };
  let finish!: () => void;
  let completed = 0;
  const first = initialize(ready, [
    () => new Promise<void>((resolve) => { finish = resolve; }),
    async () => { throw new Error("synthetic unavailable optional service"); },
    async () => { completed++; },
  ]);
  assert.equal(await initialize(ready, [async () => { completed++; }]), false);
  finish();
  assert.equal(await first, true);
  assert.equal(completed, 1);
});
