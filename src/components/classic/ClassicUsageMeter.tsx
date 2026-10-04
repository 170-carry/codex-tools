import type { UsageWindow } from "../../types/app";
import type { UiCopy } from "../accounts/types";
import { percent, remainingPercent, toProgressWidth } from "../../utils/usage";
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
  className,
}: {
  label: string;
  windowLabel: "5h" | "1w";
  window: UsageWindow | null;
  text: UiCopy;
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
        <span>{windowLabel}</span>
      </div>
      <span className="visuallyHidden">
        {label} {percent(value)} {text.remainingSuffix(percent(remaining))}
      </span>
    </div>
  );
}
