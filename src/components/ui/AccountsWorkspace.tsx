import { Presence } from "./Presence";
import type { ReactNode } from "react";

export function AccountsWorkspace({ summary, children, inspector, busy, className = "" }: {
  summary: ReactNode; children: ReactNode; inspector?: ReactNode; busy?: boolean; className?: string;
}) {
  return <div className="accountsLayout" aria-busy={busy}>
    <div className="accountsSummary">{summary}</div>
    <div className={`accountsWorkspace ${className}${inspector ? " hasInspector" : ""}`}>
      <div className="accountListStack">{children}</div>
      <Presence present={!!inspector}>{inspector}</Presence>
    </div>
  </div>;
}
