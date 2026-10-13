import assert from "node:assert/strict";
import test from "node:test";
import { sortAccountsForDisplay, moveAccountGroup } from "../src/utils/accountDisplayOrder.ts";
import type { AccountSummary } from "../src/types/app.ts";

const account = (id: string, addedAt: number, isCurrent = false) => ({ id, accountKey: id, addedAt, isCurrent, label: id } as AccountSummary);

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


test("manual order moves whole account groups without rewriting import timestamps", () => {
  const accounts = [account("a", 10), account("b", 20), { ...account("b-new", 30), accountKey: "b" }, account("c", 40)];
  const order = moveAccountGroup(accounts, [], "b", -1);
  assert.deepEqual(order, ["b", "a", "c"]);
  assert.deepEqual(sortAccountsForDisplay(accounts, order).map(a => a.id), ["b", "b-new", "a", "c"]);
  assert.deepEqual(moveAccountGroup(accounts, order, "b", -1), order);
  assert.deepEqual(moveAccountGroup(accounts, order, "missing", 1), order);
  assert.deepEqual(accounts.map(a => a.addedAt), [10, 20, 30, 40]);
});
