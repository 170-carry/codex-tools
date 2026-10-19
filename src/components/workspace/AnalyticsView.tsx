import { AnalyticsPanel } from "../AnalyticsPanel";
import type { CodexController } from "../../types/workspace";

export function AnalyticsView({ c }: { c: CodexController }) {
  return (
    <AnalyticsPanel
      analytics={c.costAnalytics}
      error={c.costAnalyticsError}
      loading={c.costAnalyticsLoading}
      exporting={c.costAnalyticsExporting}
      progress={c.costAnalyticsProgress}
      weeklyBudgetUsd={c.settings.codexAnalyticsWeeklyBudgetUsd}
      savingSettings={c.savingSettings}
      onRefresh={() => {
        void c.refreshCostAnalytics(false);
        void c.refreshTokenUsage(true);
      }}
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
