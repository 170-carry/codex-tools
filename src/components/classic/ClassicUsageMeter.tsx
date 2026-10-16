import type { UsageWindow } from "../../types/app";
import type { UiCopy } from "../accounts/types";
import { percent, remainingPercent, toProgressWidth } from "../../utils/usage";
import { formatFullDate } from "../../utils/dateFormatting";
import { formatResetShort, getAccountStatsCopy } from "./accountTokenStats";
function usedPercent(window: UsageWindow | null): number | null {
  if (!window || !Number.isFinite(window.usedPercent)) {
    return null;
  }
  return Math.max(0, Math.min(100, window.usedPercent));
}
export function UsageMeter({
  label,
  windowLabel,
  window,
  text,
  locale,
  className,
}: {
  label: string;
  windowLabel: "5h" | "1w";
  window: UsageWindow | null;
  text: UiCopy;
  locale: string;
  className?: string;
}) {
  const value = usedPercent(window);
  const remaining = value === null ? null : remainingPercent(window);
  const tone =
    remaining !== null && remaining <= 0
      ? "danger"
      : remaining !== null && remaining < 15
        ? "warning"
        : "normal";
  const reset = formatResetShort(window?.resetAt, locale);

  return (
    <div
      className={`usageMeter tone-${tone}${className ? ` ${className}` : ""}`}
    >
      <div className="usageMeterHead">
        <span>{label}</span>
        <strong>{percent(value)}</strong>
      </div>
      <div className="usageBar" aria-hidden="true">
        <span style={{ width: toProgressWidth(value) }} />
      </div>
      <div className="usageMeterFoot">
        <span>{text.remainingSuffix(percent(remaining))}</span>
        <span
          className="usageMeterReset"
          title={
            window?.resetAt
              ? `${text.resetTime} · ${formatFullDate(window.resetAt, locale, text.emptyValue)}`
              : undefined
          }
        >
          {reset ? getAccountStatsCopy(locale).resetAt(reset) : windowLabel}
        </span>
      </div>
      <span className="visuallyHidden">
        {label} {percent(value)} {text.remainingSuffix(percent(remaining))}
      </span>
    </div>
  );
}
