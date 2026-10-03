import { type MultiSelectOption } from "../EditorMultiSelect";
import type { ApiProxyKeyUsageLogEntry, RemoteAuthMode } from "../../types/app";
export function formatApiProxyKeyLogTime(
  locale: string,
  timestamp: number | null,
) {
  if (timestamp === null || timestamp <= 0) {
    return "--";
  }

  // 后端统一返回 Unix 秒；Date/Intl 接收毫秒，必须在展示边界转换。
  const timestampMs = timestamp * 1000;
  try {
    return new Intl.DateTimeFormat(locale, {
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    }).format(timestampMs);
  } catch {
    return new Date(timestampMs).toLocaleString();
  }
}

export function summarizeApiProxyKeyLogs(
  logs: ApiProxyKeyUsageLogEntry[],
  keyId: string,
) {
  return logs.reduce(
    (summary, log) => {
      if (log.keyId !== keyId) {
        return summary;
      }

      return {
        calls: summary.calls + log.calls,
        tokens: summary.tokens + log.tokens,
        lastUsedAt:
          summary.lastUsedAt === null || log.timestamp > summary.lastUsedAt
            ? log.timestamp
            : summary.lastUsedAt,
      };
    },
    { calls: 0, tokens: 0, lastUsedAt: null as number | null },
  );
}

export function toggleStringValue(
  values: string[],
  value: string,
  enabled: boolean,
) {
  if (enabled) {
    return values.includes(value) ? values : [...values, value];
  }
  return values.filter((item) => item !== value);
}

export const REMOTE_AUTH_OPTIONS: MultiSelectOption<RemoteAuthMode>[] = [
  { id: "keyContent", label: "keyContent" },
  { id: "keyFile", label: "keyFile" },
  { id: "keyPath", label: "keyPath" },
  { id: "password", label: "password" },
];
