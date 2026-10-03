import type { ReactNode } from "react";

export function PageToolbar({
  title,
  detail,
  detailTitle,
  actions,
}: {
  title: string;
  detail?: string;
  detailTitle?: string;
  actions?: ReactNode;
}) {
  return (
    <header className="pageToolbar">
      <div>
        <h2>{title}</h2>
        {detail ? <span title={detailTitle}>{detail}</span> : null}
      </div>
      {actions ? <div className="pageToolbarActions">{actions}</div> : null}
    </header>
  );
}
