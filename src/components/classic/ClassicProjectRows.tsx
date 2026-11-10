import { Pagination } from "../ui/Pagination";
import { usePagination } from "../../hooks/usePagination";
import type { CodexProjectCostBreakdown } from "../../types/app";
import { formatUsd, formatNumber } from "../analytics/formatting";
export function ClassicProjectRows({
  projects,
  locale,
}: {
  projects: CodexProjectCostBreakdown[];
  locale: string;
}) {
  const pagination = usePagination(projects, 10);
  const maxCost = Math.max(
    ...projects.map((project) => project.costUsd),
    0.000001,
  );

  return (
    <>
    <div className="analyticsProjectList">
      {pagination.items.map((project) => (
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
            {project.sessionCount} {locale === "zh-CN" ? "会话" : "sessions"} · {project.promptCount} prompts ·{" "}
            {project.eventCount} {locale === "zh-CN" ? "用量记录" : "events"}
          </small>
        </article>
      ))}
    </div>
    <Pagination {...pagination} locale={locale} />
    </>
  );
}
