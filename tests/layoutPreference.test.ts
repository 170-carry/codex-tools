import assert from "node:assert/strict";
import test from "node:test";
import {
  DEFAULT_APP_LAYOUT,
  LAYOUT_STORAGE_KEY,
  parseAppLayout,
  readAppLayout,
  saveAppLayout,
} from "../src/utils/layoutPreference.ts";

test("new installs and upgrades without a layout preference default to original", () => {
  assert.equal(DEFAULT_APP_LAYOUT, "classic");
  assert.equal(readAppLayout(), "classic");
  const priorSettings = new Map([
    ["codex-tools-theme", "dark"],
    ["codex-tools-locale", "zh-CN"],
  ]);
  assert.equal(
    readAppLayout({ getItem: (key) => priorSettings.get(key) ?? null }),
    "classic",
  );
});

test("both choices survive reloads without changing unrelated preferences", () => {
  const values = new Map([["codex-tools-theme", "dark"]]);
  const storage = {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => {
      values.set(key, value);
    },
  };
  assert.equal(saveAppLayout(storage, "compact"), true);
  assert.equal(readAppLayout(storage), "compact");
  assert.equal(saveAppLayout(storage, "classic"), true);
  assert.equal(readAppLayout(storage), "classic");
  assert.equal(values.get(LAYOUT_STORAGE_KEY), "classic");
  assert.equal(values.get("codex-tools-theme"), "dark");
  assert.equal(values.size, 2);
});

test("unsupported or damaged saved values fall back to original", () => {
  for (const value of [
    null,
    undefined,
    "",
    "native",
    "invalid",
    "COMPACT",
    {},
    1,
  ])
    assert.equal(parseAppLayout(value), "classic");
  assert.equal(parseAppLayout("compact"), "compact");
});

test("unavailable browser storage does not prevent opening the default layout", () => {
  const blocked = {
    getItem: () => {
      throw new Error("Storage blocked");
    },
    setItem: () => {
      throw new Error("Storage blocked");
    },
  };
  assert.equal(readAppLayout(blocked), "classic");
  assert.equal(saveAppLayout(blocked, "compact"), false);
  assert.equal(saveAppLayout(undefined, "classic"), false);
});
