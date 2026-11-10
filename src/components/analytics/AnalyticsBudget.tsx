import { useI18n } from "../../i18n/I18nProvider";
import { uiCopy } from "../../i18n/uiCopy";
import { notifyFeedback } from "../ui/feedback";
import { useRef, useState } from "react";
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
  const { locale } = useI18n();
  const [invalid, setInvalid] = useState(false);
  const ui = uiCopy(locale);
  const input = useRef<HTMLInputElement>(null);
  const budgetValue = weeklyBudgetUsd === null ? "" : String(weeklyBudgetUsd);
  const alert = analytics?.weeklyBudgetAlert ?? "none";
  const save = () => {
    const trimmed = input.current?.value.trim() ?? "";
    const value = trimmed === "" ? null : Number(trimmed);
    if (value !== null && (!Number.isFinite(value) || value <= 0)) { setInvalid(true); input.current?.focus(); return; }
    setInvalid(false);
    void onUpdate(value).then(() => notifyFeedback("saved")).catch(() => notifyFeedback("saveFailed"));
  };
  return (
      <section className={`analyticsBudget tone-${alert}`}>
        <div>
          <span>{text.budgetTitle}</span>
          <strong>{alertLabel(alert, text)}</strong>
          <p>{text.budgetDescription}</p>
        </div>
        <div className="analyticsBudgetMeter" aria-hidden="true">
          <i
            style={{
              width: `${Math.max(0, Math.min(100, analytics?.weeklyBudgetPercent ?? 0))}%`,
            }}
          />
        </div>
        <label>
          <span>{text.budgetInputLabel}</span>
          <input
            ref={input}
            key={budgetValue}
            defaultValue={budgetValue}
            aria-invalid={invalid}
            aria-describedby={invalid ? "budget-error" : undefined}
            onInput={() => setInvalid(false)}
            inputMode="decimal"
            placeholder={text.budgetPlaceholder}
          />
        </label>
        {invalid && <p id="budget-error" className="uiInlineError" role="alert">{ui.budgetInvalid}</p>}
        <div className="analyticsBudgetActions">
          <button
            type="button"
            className="ghost"
            disabled={saving}
            onClick={() => {
              if (input.current) input.current.value = "";
              setInvalid(false);
              void onUpdate(null).then(() => notifyFeedback("saved")).catch(() => notifyFeedback("saveFailed"));
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
      </section>
    );
}
