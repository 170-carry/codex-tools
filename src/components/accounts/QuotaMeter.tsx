import type { UsageWindow } from "../../types/app";
import { percent, remainingPercent, toProgressWidth } from "../../utils/usage";
import { QuotaResetTime } from "./QuotaResetTime";

export function QuotaMeter({
  window,
  label,
  locale,
  compact = false,
}: {
  window: UsageWindow | null;
  label: string;
  locale: string;
  compact?: boolean;
}) {
  const remaining = remainingPercent(window);
  const value =
    remaining === null || !Number.isFinite(remaining) ? null : remaining;
  const tone =
    value === null
      ? "unknown"
      : value <= 0
        ? "danger"
        : value < 15
          ? "warning"
          : "normal";
  const text =
    locale === "zh-CN"
      ? { remaining: "剩余", reset: "重置", unavailable: "额度未获取" }
      : {
          remaining: "remaining",
          reset: "Resets",
          unavailable: "Quota unavailable",
        };
  return (
    <div className={`quotaMeter tone-${tone}${compact ? " isCompact" : ""}`}>
      <div className="quotaMeterHead">
        <span>{label}</span>
        <strong>
          {percent(value)}
          {!compact ? <small>{text.remaining}</small> : null}
        </strong>
      </div>
      <div
        className="quotaTrack"
        role="progressbar"
        aria-label={`${label} · ${text.remaining}`}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={value ?? undefined}
        aria-valuetext={value === null ? text.unavailable : percent(value)}
      >
        <span style={{ width: toProgressWidth(value) }} />
      </div>
      {window ? (
        <QuotaResetTime
          resetAt={window.resetAt}
          label={`${label} · ${text.reset}`}
        />
      ) : (
        <span className="quotaReset">{text.unavailable}</span>
      )}
    </div>
  );
}
