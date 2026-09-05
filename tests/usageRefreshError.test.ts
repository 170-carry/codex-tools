import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import {
  classifyUsageRefreshError,
  extractUsageRefreshStatusCode,
  type UsageRefreshFailureKind,
} from "../src/utils/usageRefreshError.ts";

type LocaleFile = {
  accountsGrid: Record<string, string>;
};

const locale = JSON.parse(
  await readFile(new URL("../src/i18n/locales/zh-CN.json", import.meta.url), "utf8"),
) as LocaleFile;

const reasonKeyByKind: Record<UsageRefreshFailureKind, string> = {
  timeout: "usageFailureTimeout",
  network: "usageFailureNetwork",
  authorization: "usageFailureAuthorization",
  rateLimited: "usageFailureRateLimited",
  server: "usageFailureServer",
  invalidResponse: "usageFailureInvalidResponse",
  unknown: "usageFailureUnknown",
};

const fixtures: Array<{
  name: string;
  error: string;
  expectedKind: UsageRefreshFailureKind;
  expectedLabel: string;
}> = [
  {
    name: "请求超时",
    error:
      "请求用量接口失败: https://chatgpt.com/backend-api/wham/usage -> error sending request -> operation timed out",
    expectedKind: "timeout",
    expectedLabel: "更新失败：请求超时 · 显示缓存 07/22 09:54",
  },
  {
    name: "网络连接失败",
    error:
      "请求用量接口失败: https://chatgpt.com/backend-api/wham/usage -> error sending request -> dns error: failed to lookup address",
    expectedKind: "network",
    expectedLabel: "更新失败：网络连接失败 · 显示缓存 07/22 09:54",
  },
  {
    name: "401 授权异常",
    error:
      "请求用量接口失败: https://chatgpt.com/backend-api/wham/usage -> 401 Unauthorized: provided authentication token is expired",
    expectedKind: "authorization",
    expectedLabel: "更新失败：授权异常 · 显示缓存 07/22 09:54",
  },
  {
    name: "403 授权异常",
    error:
      "请求用量接口失败: https://chatgpt.com/backend-api/wham/usage -> 403 Forbidden",
    expectedKind: "authorization",
    expectedLabel: "更新失败：授权异常 · 显示缓存 07/22 09:54",
  },
  {
    name: "请求限制",
    error:
      "请求用量接口失败: https://chatgpt.com/backend-api/wham/usage -> 429 Too Many Requests: usage_limit_reached",
    expectedKind: "rateLimited",
    expectedLabel: "更新失败：已达用量或请求限制 · 显示缓存 07/22 09:54",
  },
  {
    name: "服务不可用",
    error:
      "请求用量接口失败: https://chatgpt.com/backend-api/wham/usage -> 503 Service Unavailable",
    expectedKind: "server",
    expectedLabel: "更新失败：服务暂不可用 · 显示缓存 07/22 09:54",
  },
  {
    name: "主接口 503 且备用接口返回 403 HTML",
    error:
      "请求用量接口失败: https://chatgpt.com/backend-api/wham/usage -> 503 Service Unavailable: biscuit_baker_service_me_circuit_open | https://chatgpt.com/wham/usage -> 403 Forbidden: <html>",
    expectedKind: "server",
    expectedLabel: "更新失败：服务暂不可用 · 显示缓存 07/22 09:54",
  },
  {
    name: "返回数据无效",
    error:
      "请求用量接口失败: https://chatgpt.com/backend-api/wham/usage -> 解析返回失败: expected value at line 1 column 1",
    expectedKind: "invalidResponse",
    expectedLabel: "更新失败：返回数据无效 · 显示缓存 07/22 09:54",
  },
  {
    name: "未知错误",
    error:
      "请求用量接口失败: https://chatgpt.com/backend-api/wham/usage -> unexpected upstream condition",
    expectedKind: "unknown",
    expectedLabel: "更新失败：未知错误 · 显示缓存 07/22 09:54",
  },
];

function fillTemplate(template: string, values: Record<string, string>): string {
  return Object.entries(values).reduce(
    (result, [key, value]) => result.replaceAll(`{{${key}}}`, value),
    template,
  );
}

test("组合错误优先展示主服务的 503", () => {
  const error =
    "primary -> 503 Service Unavailable: biscuit_baker_service_me_circuit_open | fallback -> 403 Forbidden: <html>";
  assert.equal(extractUsageRefreshStatusCode(error), 503);
});

test("授权错误优先展示 401 或 403", () => {
  assert.equal(extractUsageRefreshStatusCode("fallback 403 | primary 401 Unauthorized"), 401);
  assert.equal(extractUsageRefreshStatusCode("request failed with 403 Forbidden"), 403);
});

for (const fixture of fixtures) {
  test(`${fixture.name} -> ${fixture.expectedLabel}`, () => {
    const kind = classifyUsageRefreshError(fixture.error);
    const reason = locale.accountsGrid[reasonKeyByKind[kind]];
    const template = locale.accountsGrid.usageRefreshFailedCached;

    assert.equal(kind, fixture.expectedKind);
    assert.ok(reason);
    assert.ok(template);
    assert.equal(
      fillTemplate(template, { reason, updatedAt: "07/22 09:54" }),
      fixture.expectedLabel,
    );
  });
}
