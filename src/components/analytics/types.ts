import type { ReactNode } from "react";
import type { useI18n } from "../../i18n/I18nProvider";
import type {
  CodexCostAnalyticsSnapshot,
  CodexCostAnalyticsProgress,
  CodexSessionCostBreakdown,
} from "../../types/app";

export type AnalyticsPanelProps = {
  analytics: CodexCostAnalyticsSnapshot | null;
  error: string | null;
  loading: boolean;
  exporting: "csv" | "json" | null;
  progress: CodexCostAnalyticsProgress | null;
  weeklyBudgetUsd: number | null;
  savingSettings: boolean;
  tokenUsageContent?: ReactNode;
  onRefresh?: () => void;
  onExport: (format: "csv" | "json") => void;
  onDeleteSession: (session: CodexSessionCostBreakdown) => Promise<void> | void;
  onUpdateWeeklyBudget: (value: number | null) => Promise<void>;
};

export type AnalyticsCopy = ReturnType<typeof useI18n>["copy"]["analytics"];
