import assert from "node:assert/strict";
import test from "node:test";
import { sortAccountsForDisplay } from "../src/utils/accountDisplayOrder.ts";
import type { AccountSummary } from "../src/types/app.ts";

const account = (id: string, addedAt: number, isCurrent = false) => ({ id, addedAt, isCurrent, label: id } as AccountSummary);

test("display order survives quota refresh, switching and label changes", () => {
  const original = [account("b", 20), account("a", 10)];
  const refreshed = original.map((a) => ({ ...a, label: a.id === "b" ? "AAA" : "ZZZ", isCurrent: a.id === "b", usage: { oneWeek: { usedPercent: a.id === "b" ? 0 : 99 } } } as AccountSummary));
  assert.deepEqual(sortAccountsForDisplay(original).map(a => a.id), ["a", "b"]);
  assert.deepEqual(sortAccountsForDisplay(refreshed).map(a => a.id), ["a", "b"]);
  assert.deepEqual(original.map(a => a.id), ["b", "a"]);
});

test("same-second imports have a deterministic tie break", () => {
  assert.deepEqual(sortAccountsForDisplay([account("z", 10), account("a", 10)]).map(a => a.id), ["a", "z"]);
});
