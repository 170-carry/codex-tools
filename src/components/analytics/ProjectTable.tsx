import type { CodexProjectCostBreakdown } from "../../types/app";
import { getPageLayoutCopy } from "../../i18n/pageLayoutCopy";
import { formatNumber, formatUsd, formatDateTime } from "./formatting";

export function ProjectTable({
  projects,
  locale,
}: {
  projects: CodexProjectCostBreakdown[];
  locale: string;
}) {
  const text = getPageLayoutCopy(locale);
  return (
    <div className="analyticsTableWrap">
      <table className="analyticsTable analyticsProjectTable">
        <thead>
          <tr>
            <th>{text.project}</th>
            <th>{text.sessions}</th>
            <th>{text.tokens}</th>
            <th>{text.cost}</th>
            <th>{text.updated}</th>
          </tr>
        </thead>
        <tbody>
          {projects.map((project) => (
            <tr key={project.projectPath}>
              <td>
                <strong title={project.projectPath}>
                  {project.projectName}
                </strong>
                <small title={project.projectPath}>{project.projectPath}</small>
              </td>
              <td>{project.sessionCount}</td>
              <td>{formatNumber(project.total.totalTokens, locale)}</td>
              <td>{formatUsd(project.costUsd, locale)}</td>
              <td>{formatDateTime(project.lastAt, locale)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {projects.length === 0 ? (
        <p className="analyticsNoResults">{text.noResults}</p>
      ) : null}
    </div>
  );
}
