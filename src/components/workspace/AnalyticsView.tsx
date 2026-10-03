import { AnalyticsPanel } from "../AnalyticsPanel";
import type { CodexController } from "../../types/workspace";
import { TokenUsageStrip } from "../accounts/TokenUsageStrip";
import { getUiCopy } from "../accounts/accountCopy";
import { useI18n } from "../../i18n/I18nProvider";

export function AnalyticsView({ c }: { c: CodexController }) {
  const { locale } = useI18n();
  return (
    <AnalyticsPanel
      tokenUsageContent={
        <TokenUsageStrip
          tokenUsage={c.tokenUsage}
          error={c.tokenUsageError}
          text={getUiCopy(locale)}
        />
      }
      analytics={c.costAnalytics}
      error={c.costAnalyticsError}
      loading={c.costAnalyticsLoading}
      exporting={c.costAnalyticsExporting}
      progress={c.costAnalyticsProgress}
      weeklyBudgetUsd={c.settings.codexAnalyticsWeeklyBudgetUsd}
      savingSettings={c.savingSettings}
      onExport={(format) => void c.exportCostAnalytics(format)}
      onDeleteSession={(session) => void c.onDeleteCodexSession(session)}
      onUpdateWeeklyBudget={(value) =>
        c
          .updateSettings(
            { codexAnalyticsWeeklyBudgetUsd: value },
            { silent: true, keepInteractive: true },
          )
          .then(async () => {
            await c.loadCostAnalytics(true);
          })
      }
    />
  );
}
