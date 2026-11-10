import type { ReactNode } from "react";
export function PageFrame({ kind, className = "", children }: { kind: "list" | "analysis" | "settings" | "service"; className?: string; children: ReactNode }) {
  return <section className={`uiPage uiPage-${kind} ${className}`}>{children}</section>;
}
export function PageHeader({ title, context, actions }: { title: string; context?: ReactNode; actions?: ReactNode }) {
  return <header className="uiPageHeader"><div><h2>{title}</h2>{context && <div className="uiPageContext">{context}</div>}</div>{actions && <div className="uiPageActions">{actions}</div>}</header>;
}
export function SectionGroup({ title, actions, children, className = "" }: { title: string; actions?: ReactNode; children: ReactNode; className?: string }) {
  return <section className={`uiSection ${className}`} aria-label={title}><header className="uiSectionHeader"><h3>{title}</h3>{actions}</header><div className="uiSectionBody">{children}</div></section>;
}
