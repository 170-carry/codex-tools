export type UsageRefreshFailureKind =
  | "timeout"
  | "network"
  | "authorization"
  | "rateLimited"
  | "server"
  | "invalidResponse"
  | "unknown";

export function classifyUsageRefreshError(error: string): UsageRefreshFailureKind {
  const normalized = error.toLocaleLowerCase();

  if (
    normalized.includes("timeout") ||
    normalized.includes("timed out") ||
    normalized.includes("超时")
  ) {
    return "timeout";
  }
  if (
    /\b429\b/.test(normalized) ||
    normalized.includes("too many requests") ||
    normalized.includes("rate limit") ||
    normalized.includes("usage_limit_reached") ||
    normalized.includes("请求过于频繁") ||
    normalized.includes("用量限制")
  ) {
    return "rateLimited";
  }
  if (
    /\b5\d\d\b/.test(normalized) ||
    normalized.includes("service unavailable") ||
    normalized.includes("bad gateway") ||
    normalized.includes("internal server error") ||
    normalized.includes("circuit_open") ||
    normalized.includes("服务不可用")
  ) {
    return "server";
  }
  if (
    /\b(?:401|403)\b/.test(normalized) ||
    normalized.includes("unauthorized") ||
    normalized.includes("forbidden") ||
    normalized.includes("invalid_grant") ||
    normalized.includes("access token") ||
    normalized.includes("refresh token") ||
    normalized.includes("authorization") ||
    normalized.includes("authentication") ||
    normalized.includes("授权") ||
    normalized.includes("令牌") ||
    normalized.includes("重新登录") ||
    normalized.includes("账号被封禁") ||
    normalized.includes("deactivated") ||
    normalized.includes("account blocked")
  ) {
    return "authorization";
  }
  if (
    normalized.includes("parse") ||
    normalized.includes("json") ||
    normalized.includes("invalid response") ||
    normalized.includes("解析返回失败") ||
    normalized.includes("返回数据")
  ) {
    return "invalidResponse";
  }
  if (
    normalized.includes("network") ||
    normalized.includes("connection") ||
    normalized.includes("connect") ||
    normalized.includes("dns") ||
    normalized.includes("tcp") ||
    normalized.includes("tls") ||
    normalized.includes("certificate") ||
    normalized.includes("error sending request") ||
    normalized.includes("请求错误") ||
    normalized.includes("网络") ||
    normalized.includes("连接")
  ) {
    return "network";
  }

  return "unknown";
}

export function extractUsageRefreshStatusCode(
  error: string,
  kind: UsageRefreshFailureKind = classifyUsageRefreshError(error),
): number | null {
  const codes = Array.from(error.matchAll(/\b([45]\d{2})\b/g), (match) =>
    Number(match[1]),
  );
  if (codes.length === 0) {
    return null;
  }

  if (kind === "server") {
    return codes.find((code) => code >= 500) ?? codes[0];
  }
  if (kind === "rateLimited") {
    return codes.find((code) => code === 429) ?? codes[0];
  }
  if (kind === "authorization") {
    return codes.find((code) => code === 401) ??
      codes.find((code) => code === 403) ??
      codes[0];
  }

  return codes[0];
}
