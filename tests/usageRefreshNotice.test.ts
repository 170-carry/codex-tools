import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import type { AccountSummary } from "../src/types/app.ts";
import { buildUsageRefreshNotice } from "../src/utils/usageRefreshNotice.ts";

type Copy = Parameters<typeof buildUsageRefreshNotice>[1];
type RawCopy = {
  accountsGrid: Copy["accountsGrid"];
  notices: {
    usageRefreshed: string;
    usageRefreshPartial: string;
    usageRefreshAllFailed: string;
  };
};

const locales = ["zh-CN", "en-US", "ja-JP", "ko-KR", "ru-RU"];
const copies = await Promise.all(locales.map(async (locale) => {
  const raw = JSON.parse(await readFile(
    new URL(`../src/i18n/locales/${locale}.json`, import.meta.url), "utf8",
  )) as RawCopy;
  const fill = (template: string, values: Record<string, string | number>) =>
    template.replace(/\{\{\s*(\w+)\s*\}\}/g, (_, key: string) => String(values[key]));
  return {
    accountsGrid: raw.accountsGrid,
    notices: {
      usageRefreshed: raw.notices.usageRefreshed,
      usageRefreshPartial: (failed: number, total: number, error: string) =>
        fill(raw.notices.usageRefreshPartial, { failed, total, error }),
      usageRefreshAllFailed: (total: number, error: string) =>
        fill(raw.notices.usageRefreshAllFailed, { total, error }),
    },
  } satisfies Copy;
}));
const copy = copies[0];

function account(id: string, usageError: string | null = null): AccountSummary {
  return {
    id, accountKey: id, sourceKind: "chatgpt", usageError,
    usage: { fetchedAt: 1_000, oneWeek: { usedPercent: 12 } },
  } as AccountSummary;
}

test("a successful command with one failed quota account reports partial failure and retains cache", () => {
  const accounts = [account("fresh"), account("cached", "operation timed out")];
  const before = structuredClone(accounts);
  assert.deepEqual(buildUsageRefreshNotice(accounts, copy), {
    type: "error",
    message: "用量刷新部分失败（1/2）：请求超时；失败账号保留已有缓存",
  });
  assert.deepEqual(accounts, before);
});

test("all account failures report failure even when cached usage is present", () => {
  assert.deepEqual(buildUsageRefreshNotice([
    account("cached-a", "DNS lookup failed"),
    account("cached-b", "network connection failed"),
  ], copy), {
    type: "error",
    message: "用量刷新失败（2 个账号）：网络连接失败；保留已有缓存",
  });
});

test("plan variants share one quota request and relay account errors are excluded", () => {
  const variant = { ...account("variant"), accountKey: "same" };
  const failed = { ...account("primary", "operation timed out"), accountKey: "same" };
  const relay = { ...account("relay", "relay balance failed"), sourceKind: "relay" as const };
  for (const accounts of [[variant, failed, relay], [failed, variant, relay]]) {
    assert.equal(buildUsageRefreshNotice(accounts, copy).message,
      "用量刷新失败（1 个账号）：请求超时；保留已有缓存");
  }
});

test("successful and empty quota refreshes retain the success notice", () => {
  const relay = { ...account("relay", "relay balance failed"), sourceKind: "relay" as const };
  for (const accounts of [[], [account("ok", "   ")], [relay]]) {
    assert.deepEqual(buildUsageRefreshNotice(accounts, copy), {
      type: "ok", message: copy.notices.usageRefreshed,
    });
  }
});

test("errors use existing reason classification and primary service status without raw backend text", () => {
  const error = "primary -> 503 Service Unavailable: circuit_open | fallback -> 403 Forbidden: <html>";
  assert.equal(buildUsageRefreshNotice([account("upstream", error)], copy).message,
    "用量刷新失败（1 个账号）：服务暂不可用 (503)；保留已有缓存");
});

test("every locale formats partial and complete failures with their counts and reasons", () => {
  for (const localized of copies) {
    const partial = buildUsageRefreshNotice([account("ok"), account("bad", "timeout")], localized);
    const allFailed = buildUsageRefreshNotice([account("bad", "timeout")], localized);
    assert.equal(partial.type, "error");
    assert.match(partial.message, /1\/2/);
    assert.ok(partial.message.includes(localized.accountsGrid.usageFailureTimeout));
    assert.equal(allFailed.type, "error");
    assert.ok(allFailed.message.includes(localized.accountsGrid.usageFailureTimeout));
    assert.ok(!partial.message.includes("{{") && !allFailed.message.includes("{{"));
  }
});
