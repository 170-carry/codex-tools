import { useEffect, useMemo, useRef, useState } from "react";
import { useI18n } from "../i18n/I18nProvider";
import type {
  CodexBudgetAlert,
  CodexCostAnalyticsProgress,
  CodexCostAnalyticsSnapshot,
  CodexHourlyCostBucket,
  CodexProjectCostBreakdown,
  CodexPromptCostBreakdown,
  CodexSessionCostBreakdown,
} from "../types/app";

type AnalyticsPanelProps = {
  analytics: CodexCostAnalyticsSnapshot | null;
  error: string | null;
  loading: boolean;
  exporting: "csv" | "json" | null;
  progress: CodexCostAnalyticsProgress | null;
  weeklyBudgetUsd: number | null;
  savingSettings: boolean;
  onRefresh: () => void;
  onExport: (format: "csv" | "json") => void;
  onDeleteSession: (session: CodexSessionCostBreakdown) => Promise<void> | void;
  onUpdateWeeklyBudget: (value: number | null) => Promise<void>;
};

const WEEKDAY_LABELS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

function formatUsd(value: number, locale: string) {
  const digits = Math.abs(value) < 1 ? 4 : 2;
  return new Intl.NumberFormat(locale, {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(value);
}

function formatNumber(value: number, locale: string) {
  return new Intl.NumberFormat(locale, {
    notation: "compact",
    maximumFractionDigits: 1,
  }).format(value);
}

function formatDateTime(value: number | null, locale: string) {
  if (!value) {
    return "--";
  }
  return new Intl.DateTimeFormat(locale, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value * 1000));
}

function formatDuration(seconds: number | null, locale: string) {
  if (seconds === null) {
    return "--";
  }
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours <= 0) {
    return new Intl.NumberFormat(locale).format(minutes) + "m";
  }
  return `${new Intl.NumberFormat(locale).format(hours)}h ${minutes}m`;
}

function alertLabel(
  alert: CodexBudgetAlert,
  copy: ReturnType<typeof useI18n>["copy"]["analytics"],
) {
  if (alert === "danger") {
    return copy.budgetDanger;
  }
  if (alert === "warning") {
    return copy.budgetWarning;
  }
  if (alert === "ok") {
    return copy.budgetOk;
  }
  return copy.budgetUnset;
}

function statCard(label: string, value: string, detail?: string) {
  return (
    <article className="analyticsStatCard">
      <span>{label}</span>
      <strong>{value}</strong>
      {detail ? <small>{detail}</small> : null}
    </article>
  );
}

function progressStageLabel(
  progress: CodexCostAnalyticsProgress | null,
  copy: ReturnType<typeof useI18n>["copy"]["analytics"],
) {
  if (progress?.stage === "caching") {
    return copy.progressCaching;
  }
  if (progress?.stage === "complete") {
    return copy.progressComplete;
  }
  return copy.progressScanning;
}

function Heatmap({
  buckets,
  locale,
}: {
  buckets: CodexHourlyCostBucket[];
  locale: string;
}) {
  const byKey = new Map(
    buckets.map((bucket) => [`${bucket.weekday}:${bucket.hour}`, bucket]),
  );
  const maxTokens = Math.max(...buckets.map((bucket) => bucket.tokens), 1);
  const hourLabels = Array.from({ length: 24 }, (_, hour) => hour);

  return (
    <div
      className="analyticsHeatmap"
      role="img"
      aria-label="Codex token activity heatmap"
    >
      <div className="analyticsHeatmapHeader" aria-hidden="true">
        <span />
        {hourLabels.map((hour) => (
          <b key={hour}>{hour % 6 === 0 ? hour : ""}</b>
        ))}
      </div>
      {WEEKDAY_LABELS.map((label, weekday) => (
        <div key={label} className="analyticsHeatmapRow">
          <span>{label}</span>
          {hourLabels.map((hour) => {
            const bucket = byKey.get(`${weekday}:${hour}`);
            const intensity = bucket
              ? Math.max(0.08, bucket.tokens / maxTokens)
              : 0;
            const title = `${label} ${hour}:00, ${formatNumber(bucket?.tokens ?? 0, locale)} tokens`;
            return (
              <i
                key={hour}
                title={title}
                style={{
                  opacity: intensity === 0 ? 0.18 : 0.24 + intensity * 0.76,
                }}
              />
            );
          })}
        </div>
      ))}
    </div>
  );
}

function ProjectRows({
  projects,
  locale,
}: {
  projects: CodexProjectCostBreakdown[];
  locale: string;
}) {
  const maxCost = Math.max(
    ...projects.map((project) => project.costUsd),
    0.000001,
  );

  return (
    <div className="analyticsProjectList">
      {projects.slice(0, 10).map((project) => (
        <article key={project.projectPath} className="analyticsProjectRow">
          <div>
            <strong title={project.projectPath}>{project.projectName}</strong>
            <span title={project.projectPath}>{project.projectPath}</span>
          </div>
          <div className="analyticsProjectMetrics">
            <b>{formatUsd(project.costUsd, locale)}</b>
            <small>
              {formatNumber(project.total.totalTokens, locale)} tokens
            </small>
          </div>
          <div className="analyticsProjectBar" aria-hidden="true">
            <i
              style={{
                width: `${Math.max(4, (project.costUsd / maxCost) * 100)}%`,
              }}
            />
          </div>
          <small>
            {project.sessionCount} sessions · {project.promptCount} prompts ·{" "}
            {project.eventCount} events
          </small>
        </article>
      ))}
    </div>
  );
}

function SessionTable({
  sessions,
  locale,
  text,
  pendingDeleteSessionId,
  deletingSessionId,
  onDeleteSession,
}: {
  sessions: CodexSessionCostBreakdown[];
  locale: string;
  text: ReturnType<typeof useI18n>["copy"]["analytics"];
  pendingDeleteSessionId: string | null;
  deletingSessionId: string | null;
  onDeleteSession: (session: CodexSessionCostBreakdown) => void;
}) {
  return (
    <div className="analyticsTableWrap">
      <table className="analyticsTable">
        <thead>
          <tr>
            <th>Session</th>
            <th>Project</th>
            <th>Model</th>
            <th>Tokens</th>
            <th>Cost</th>
            <th>Updated</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {sessions.slice(0, 80).map((session) => (
            <tr key={session.sessionId}>
              <td>
                <strong title={session.sessionId}>
                  {session.sessionId.slice(0, 8)}
                </strong>
                {session.parentSessionId ? (
                  <small>parent {session.parentSessionId.slice(0, 8)}</small>
                ) : null}
              </td>
              <td title={session.projectPath}>{session.projectName}</td>
              <td>{session.model}</td>
              <td>{formatNumber(session.total.totalTokens, locale)}</td>
              <td>{formatUsd(session.costUsd, locale)}</td>
              <td>
                {formatDateTime(session.updatedAt, locale)}
                <small>{formatDuration(session.durationSeconds, locale)}</small>
              </td>
              <td>
                <button
                  type="button"
                  className="analyticsDeleteButton"
                  disabled={deletingSessionId !== null}
                  onClick={() => onDeleteSession(session)}
                >
                  {deletingSessionId === session.sessionId
                    ? text.sessionDeleting
                    : pendingDeleteSessionId === session.sessionId
                      ? text.sessionDeleteConfirm
                      : text.sessionDelete}
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function TopPrompts({
  prompts,
  locale,
}: {
  prompts: CodexPromptCostBreakdown[];
  locale: string;
}) {
  return (
    <div className="analyticsPromptList">
      {prompts.map((prompt, index) => (
        <article
          key={`${prompt.sessionId}-${prompt.timestamp}-${index}`}
          className="analyticsPromptRow"
        >
          <div className="analyticsPromptRank">{index + 1}</div>
          <div className="analyticsPromptBody">
            <strong>{formatUsd(prompt.costUsd, locale)}</strong>
            <p title={prompt.promptPreview}>{prompt.promptPreview}</p>
            <span>
              {prompt.projectName} · {prompt.model} ·{" "}
              {formatNumber(prompt.total.totalTokens, locale)} tokens ·{" "}
              {prompt.promptChars} chars
            </span>
          </div>
        </article>
      ))}
    </div>
  );
}

export function AnalyticsPanel({
  analytics,
  error,
  loading,
  exporting,
  progress,
  weeklyBudgetUsd,
  savingSettings,
  onRefresh,
  onExport,
  onDeleteSession,
  onUpdateWeeklyBudget,
}: AnalyticsPanelProps) {
  const { copy, locale } = useI18n();
  const text = copy.analytics;
  const budgetInputRef = useRef<HTMLInputElement | null>(null);
  const deleteConfirmTimerRef = useRef<number | null>(null);
  const [sessionQuery, setSessionQuery] = useState("");
  const [pendingDeleteSessionId, setPendingDeleteSessionId] = useState<
    string | null
  >(null);
  const [deletingSessionId, setDeletingSessionId] = useState<string | null>(
    null,
  );
  const budgetInputValue =
    weeklyBudgetUsd === null ? "" : String(weeklyBudgetUsd);

  const normalizedQuery = sessionQuery.trim().toLocaleLowerCase();
  const filteredSessions = useMemo(() => {
    const sessions = analytics?.sessions ?? [];
    if (!normalizedQuery) {
      return sessions;
    }
    return sessions.filter((session) =>
      [
        session.sessionId,
        session.parentSessionId ?? "",
        session.projectName,
        session.projectPath,
        session.model,
      ]
        .join(" ")
        .toLocaleLowerCase()
        .includes(normalizedQuery),
    );
  }, [analytics?.sessions, normalizedQuery]);

  const saveBudget = () => {
    const trimmed = budgetInputRef.current?.value.trim() ?? "";
    const value = trimmed === "" ? null : Number(trimmed);
    if (value !== null && (!Number.isFinite(value) || value <= 0)) {
      return;
    }
    void onUpdateWeeklyBudget(value);
  };

  const clearBudget = () => {
    if (budgetInputRef.current) {
      budgetInputRef.current.value = "";
    }
    void onUpdateWeeklyBudget(null);
  };

  const clearDeleteConfirmTimer = () => {
    if (deleteConfirmTimerRef.current !== null) {
      window.clearTimeout(deleteConfirmTimerRef.current);
      deleteConfirmTimerRef.current = null;
    }
  };

  const handleDeleteSession = (session: CodexSessionCostBreakdown) => {
    if (deletingSessionId !== null) {
      return;
    }

    if (pendingDeleteSessionId !== session.sessionId) {
      clearDeleteConfirmTimer();
      setPendingDeleteSessionId(session.sessionId);
      deleteConfirmTimerRef.current = window.setTimeout(() => {
        setPendingDeleteSessionId((current) =>
          current === session.sessionId ? null : current,
        );
        deleteConfirmTimerRef.current = null;
      }, 3_000);
      return;
    }

    clearDeleteConfirmTimer();
    setDeletingSessionId(session.sessionId);
    void Promise.resolve(onDeleteSession(session))
      .catch(() => {})
      .finally(() => {
        setPendingDeleteSessionId(null);
        setDeletingSessionId(null);
      });
  };

  useEffect(
    () => () => {
      if (deleteConfirmTimerRef.current !== null) {
        window.clearTimeout(deleteConfirmTimerRef.current);
      }
    },
    [],
  );

  const budgetPercent = analytics?.weeklyBudgetPercent ?? null;
  const hasData = analytics !== null && analytics.eventCount > 0;
  const showProgress = loading || progress !== null;
  const progressPercent = Math.max(
    0,
    Math.min(100, Math.round(progress?.percent ?? (loading ? 6 : 0))),
  );
  const progressFiles =
    progress && progress.totalFiles > 0
      ? `${formatNumber(progress.processedFiles, locale)} / ${formatNumber(progress.totalFiles, locale)} ${text.sourceFiles}`
      : text.loadingDescription;

  return (
    <section className="analyticsPage">
      <div className="analyticsShell">
        <header className="analyticsHeader">
          <div>
            <span className="analyticsKicker">{text.kicker}</span>
            <h2>{text.title}</h2>
            <p>{text.description}</p>
          </div>
          <div className="analyticsActions">
            <button
              type="button"
              className="ghost"
              onClick={onRefresh}
              disabled={loading}
            >
              {text.refresh}
            </button>
            <button
              type="button"
              className="ghost"
              onClick={() => onExport("csv")}
              disabled={exporting !== null}
            >
              {exporting === "csv" ? text.exporting : text.exportCsv}
            </button>
            <button
              type="button"
              className="primary"
              onClick={() => onExport("json")}
              disabled={exporting !== null}
            >
              {exporting === "json" ? text.exporting : text.exportJson}
            </button>
          </div>
        </header>

        {error ? (
          <section className="analyticsNotice tone-danger">
            <strong>{text.errorTitle}</strong>
            <span>{error}</span>
          </section>
        ) : null}

        {showProgress ? (
          <section className="analyticsProgress" aria-live="polite">
            <div>
              <strong>{progressStageLabel(progress, text)}</strong>
              <span>{progressFiles}</span>
            </div>
            <div
              className="analyticsProgressMeter"
              aria-label={`${progressPercent}%`}
            >
              <i style={{ width: `${progressPercent}%` }} />
            </div>
            <b>{progressPercent}%</b>
            {progress?.currentPath ? (
              <code title={progress.currentPath}>{progress.currentPath}</code>
            ) : null}
          </section>
        ) : null}

        <section className="analyticsStats">
          {statCard(
            text.totalCost,
            analytics ? formatUsd(analytics.totalCostUsd, locale) : "--",
            text.pricingEstimate,
          )}
          {statCard(
            text.last7dCost,
            analytics ? formatUsd(analytics.last7dCostUsd, locale) : "--",
            `${analytics?.weeklyBudgetPercent ?? 0}%`,
          )}
          {statCard(
            text.totalTokens,
            analytics
              ? formatNumber(analytics.total.totalTokens, locale)
              : "--",
            text.tokenEvents,
          )}
          {statCard(
            text.sessions,
            analytics ? formatNumber(analytics.sessions.length, locale) : "--",
            text.sourceFiles,
          )}
        </section>

        <section
          className={`analyticsBudget tone-${analytics?.weeklyBudgetAlert ?? "none"}`}
        >
          <div>
            <span>{text.budgetTitle}</span>
            <strong>
              {analytics
                ? alertLabel(analytics.weeklyBudgetAlert, text)
                : text.budgetUnset}
            </strong>
            <p>{text.budgetDescription}</p>
          </div>
          <div className="analyticsBudgetMeter" aria-hidden="true">
            <i
              style={{
                width: `${Math.min(100, Math.max(0, budgetPercent ?? 0))}%`,
              }}
            />
          </div>
          <label>
            <span>{text.budgetInputLabel}</span>
            <input
              key={budgetInputValue}
              ref={budgetInputRef}
              defaultValue={budgetInputValue}
              inputMode="decimal"
              placeholder={text.budgetPlaceholder}
            />
          </label>
          <div className="analyticsBudgetActions">
            <button
              type="button"
              className="ghost"
              onClick={clearBudget}
              disabled={savingSettings}
            >
              {text.budgetClear}
            </button>
            <button
              type="button"
              className="primary"
              onClick={saveBudget}
              disabled={savingSettings}
            >
              {text.budgetSave}
            </button>
          </div>
        </section>

        {loading && !analytics ? (
          <section className="analyticsEmpty">
            <strong>{text.loadingTitle}</strong>
            <span>{text.loadingDescription}</span>
          </section>
        ) : !hasData ? (
          <section className="analyticsEmpty">
            <strong>{text.emptyTitle}</strong>
            <span>{text.emptyDescription}</span>
          </section>
        ) : analytics ? (
          <div className="analyticsGrid">
            <section className="analyticsBlock analyticsBlockProjects">
              <div className="analyticsBlockHead">
                <div>
                  <h3>{text.projectsTitle}</h3>
                  <p>{text.projectsDescription}</p>
                </div>
              </div>
              <ProjectRows projects={analytics.projects} locale={locale} />
            </section>

            <section className="analyticsBlock analyticsBlockHeatmap">
              <div className="analyticsBlockHead">
                <div>
                  <h3>{text.heatmapTitle}</h3>
                  <p>{text.heatmapDescription}</p>
                </div>
              </div>
              <Heatmap buckets={analytics.heatmap} locale={locale} />
            </section>

            <section className="analyticsBlock analyticsBlockSessions">
              <div className="analyticsBlockHead">
                <div>
                  <h3>{text.sessionsTitle}</h3>
                  <p>{text.sessionsDescription}</p>
                </div>
                <input
                  className="analyticsSearch"
                  value={sessionQuery}
                  placeholder="Search sessions"
                  onChange={(event) => setSessionQuery(event.target.value)}
                />
              </div>
              <SessionTable
                sessions={filteredSessions}
                locale={locale}
                text={text}
                pendingDeleteSessionId={pendingDeleteSessionId}
                deletingSessionId={deletingSessionId}
                onDeleteSession={handleDeleteSession}
              />
            </section>

            <section className="analyticsBlock analyticsBlockPrompts">
              <div className="analyticsBlockHead">
                <div>
                  <h3>{text.topPromptsTitle}</h3>
                  <p>{text.topPromptsDescription}</p>
                </div>
              </div>
              <TopPrompts prompts={analytics.topPrompts} locale={locale} />
            </section>
          </div>
        ) : null}

        {analytics ? (
          <footer className="analyticsFoot">
            <span>
              {text.updated}: {formatDateTime(analytics.updatedAt, locale)}
            </span>
            <span>
              {text.sourceFiles}: {analytics.sourcePathCount}
            </span>
            <span>
              {text.failedSources}: {analytics.failedPathCount}
            </span>
            <span>{analytics.pricingSource}</span>
          </footer>
        ) : null}
      </div>
    </section>
  );
}
