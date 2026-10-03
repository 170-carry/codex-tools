import { useMemo, useState } from "react";
import { useI18n } from "../i18n/I18nProvider";
import { getPageLayoutCopy } from "../i18n/pageLayoutCopy";
import { PageToolbar } from "./workspace/PageToolbar";
import { PageSections } from "./workspace/PageSections";
import { ActionMenu } from "./workspace/ActionMenu";
import { AnalyticsSummary } from "./analytics/AnalyticsSummary";
import { AnalyticsBudget } from "./analytics/AnalyticsBudget";
import { ProjectTable } from "./analytics/ProjectTable";
import { UsageHeatmap } from "./analytics/UsageHeatmap";
import { SessionTable } from "./analytics/SessionTable";
import { TopPrompts } from "./analytics/TopPrompts";
import { useSessionActions } from "./analytics/useSessionActions";
import {
  costSourceDetail,
  formatDateTime,
  formatNumber,
  progressStageLabel,
} from "./analytics/formatting";
import type { AnalyticsPanelProps } from "./analytics/types";

export function AnalyticsPanel({
  analytics,
  error,
  loading,
  exporting,
  progress,
  weeklyBudgetUsd,
  savingSettings,
  onExport,
  onDeleteSession,
  onUpdateWeeklyBudget,
  tokenUsageContent,
}: AnalyticsPanelProps) {
  const { copy, locale } = useI18n();
  const text = copy.analytics;
  const layout = getPageLayoutCopy(locale);
  const [sessionQuery, setSessionQuery] = useState("");
  const sessionActions = useSessionActions(onDeleteSession);
  const sessions = useMemo(() => {
    const search = sessionQuery.trim().toLocaleLowerCase();
    return (analytics?.sessions ?? []).filter(
      (session) =>
        !search ||
        [
          session.sessionId,
          session.parentSessionId,
          session.projectName,
          session.projectPath,
          session.model,
        ]
          .filter(Boolean)
          .join(" ")
          .toLocaleLowerCase()
          .includes(search),
    );
  }, [analytics?.sessions, sessionQuery]);
  const percent = Math.max(
    0,
    Math.min(100, Math.round(progress?.percent ?? (loading ? 6 : 0))),
  );
  const source = costSourceDetail(analytics, text, locale);
  const sections = [
    {
      id: "projects",
      label: layout.projects,
      content: (
        <section className="analyticsBlock analyticsBlockProjects">
          <p className="pageSectionDescription">{text.projectsDescription}</p>
          <ProjectTable projects={analytics?.projects ?? []} locale={locale} />
        </section>
      ),
    },
    {
      id: "sessions",
      label: layout.sessions,
      content: (
        <section className="analyticsBlock analyticsBlockSessions">
          <div className="analyticsSessionTools">
            <input
              className="analyticsSearch"
              aria-label={layout.searchSessions}
              placeholder={layout.searchSessions}
              value={sessionQuery}
              onChange={(event) => setSessionQuery(event.target.value)}
            />
            <span>
              {sessions.length} {layout.sessions}
            </span>
          </div>
          <SessionTable
            sessions={sessions}
            locale={locale}
            text={text}
            pendingDeleteSessionId={sessionActions.pendingDeleteSessionId}
            deletingSessionId={sessionActions.deletingSessionId}
            onDeleteSession={sessionActions.handleDeleteSession}
          />
          {sessions.length === 0 ? (
            <p className="analyticsNoResults">{layout.noResults}</p>
          ) : null}
        </section>
      ),
    },
    {
      id: "activity",
      label: layout.activity,
      content: (
        <section className="analyticsBlock analyticsBlockHeatmap">
          <p className="pageSectionDescription">{text.heatmapDescription}</p>
          <UsageHeatmap
            buckets={analytics?.heatmap ?? []}
            locale={locale}
            copy={text}
          />
        </section>
      ),
    },
    {
      id: "prompts",
      label: layout.prompts,
      content: (
        <section className="analyticsBlock analyticsBlockPrompts">
          <p className="pageSectionDescription">{text.topPromptsDescription}</p>
          <TopPrompts prompts={analytics?.topPrompts ?? []} locale={locale} />
          {!analytics?.topPrompts.length ? (
            <p className="analyticsNoResults">{layout.noResults}</p>
          ) : null}
        </section>
      ),
    },
  ];
  return (
    <section className="analyticsPage workspacePage">
      <PageToolbar
        title={layout.analytics}
        detail={source.label}
        detailTitle={source.title}
        actions={
          <ActionMenu
            label={exporting ? text.exporting : layout.export}
            disabled={exporting !== null}
            actions={[
              {
                label: text.exportCsv,
                icon: "export",
                onClick: () => onExport("csv"),
              },
              {
                label: text.exportJson,
                icon: "export",
                onClick: () => onExport("json"),
              },
            ]}
          />
        }
      />
      {error ? (
        <div className="analyticsNotice tone-danger" role="alert">
          <strong>{text.errorTitle}</strong>
          <span>{error}</span>
        </div>
      ) : null}
      {loading || progress ? (
        <div className="analyticsProgress" aria-live="polite">
          <div>
            <strong>{progressStageLabel(progress, text)}</strong>
            <span>
              {progress && progress.totalFiles > 0
                ? `${formatNumber(progress.processedFiles, locale)} / ${formatNumber(progress.totalFiles, locale)} ${text.sourceFiles}`
                : text.loadingDescription}
            </span>
          </div>
          <div className="analyticsProgressMeter" aria-label={`${percent}%`}>
            <i style={{ width: `${percent}%` }} />
          </div>
          <b>{percent}%</b>
          {progress?.currentPath ? (
            <code title={progress.currentPath}>{progress.currentPath}</code>
          ) : null}
        </div>
      ) : null}
      <AnalyticsSummary analytics={analytics} text={text} locale={locale} />
      {tokenUsageContent ? (
        <div className="analyticsTokenStrip">{tokenUsageContent}</div>
      ) : null}
      <AnalyticsBudget
        analytics={analytics}
        weeklyBudgetUsd={weeklyBudgetUsd}
        saving={savingSettings}
        onUpdate={onUpdateWeeklyBudget}
        text={text}
      />
      <PageSections label={layout.analytics} sections={sections} />
      {!loading && (!analytics || analytics.eventCount === 0) ? (
        <p className="analyticsNoResults">{text.emptyDescription}</p>
      ) : null}
      {analytics ? (
        <footer className="analyticsFoot">
          <span>
            {text.updated}: {formatDateTime(analytics.updatedAt, locale)}
          </span>
          <span>
            {text.sourceFiles}: {analytics.sourcePathCount}
          </span>
          {analytics.failedPathCount > 0 ? (
            <span>
              {text.failedSources}: {analytics.failedPathCount}
            </span>
          ) : null}
          {analytics.unresolvedForkCount > 0 ? (
            <span>
              {text.unresolvedForks}: {analytics.unresolvedForkCount}
            </span>
          ) : null}
          {analytics.unresolvedUsageEventCount > 0 ? (
            <span>
              {text.usageAnomalies}: {analytics.unresolvedUsageEventCount}
            </span>
          ) : null}
          <span title={analytics.pricingSource}>{text.pricingEstimate}</span>
        </footer>
      ) : null}
    </section>
  );
}
