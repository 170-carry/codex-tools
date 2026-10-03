import { useRef } from "react";
import type { CodexCostAnalyticsSnapshot } from "../../types/app";
import type { AnalyticsCopy } from "./types";
import { alertLabel } from "./formatting";

export function AnalyticsBudget({
  analytics,
  weeklyBudgetUsd,
  saving,
  onUpdate,
  text,
}: {
  analytics: CodexCostAnalyticsSnapshot | null;
  weeklyBudgetUsd: number | null;
  saving: boolean;
  onUpdate: (value: number | null) => Promise<void>;
  text: AnalyticsCopy;
}) {
  const input = useRef<HTMLInputElement>(null);
  const budgetValue = weeklyBudgetUsd === null ? "" : String(weeklyBudgetUsd);
  const alert = analytics?.weeklyBudgetAlert ?? "none";
  const save = () => {
    const trimmed = input.current?.value.trim() ?? "";
    const value = trimmed === "" ? null : Number(trimmed);
    if (value !== null && (!Number.isFinite(value) || value <= 0)) return;
    void onUpdate(value);
  };
  return (
    <details
      className={`analyticsBudgetControl tone-${alert}`}
      open={alert === "danger" || alert === "warning"}
    >
      <summary>
        <span>{text.budgetTitle}</span>
        <strong>{alertLabel(alert, text)}</strong>
        <span className="analyticsBudgetMiniTrack" aria-hidden="true">
          <i
            style={{
              width: `${Math.max(0, Math.min(100, analytics?.weeklyBudgetPercent ?? 0))}%`,
            }}
          />
        </span>
      </summary>
      <div className="analyticsBudgetEditor">
        <p>{text.budgetDescription}</p>
        <div>
          <label>
            <span>{text.budgetInputLabel}</span>
            <input
              ref={input}
              key={budgetValue}
              defaultValue={budgetValue}
              inputMode="decimal"
              placeholder={text.budgetPlaceholder}
            />
          </label>
          <button
            type="button"
            className="ghost"
            disabled={saving}
            onClick={() => {
              if (input.current) input.current.value = "";
              void onUpdate(null);
            }}
          >
            {text.budgetClear}
          </button>
          <button
            type="button"
            className="primary"
            disabled={saving}
            onClick={save}
          >
            {text.budgetSave}
          </button>
        </div>
      </div>
    </details>
  );
}
