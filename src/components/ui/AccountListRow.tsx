import type { ReactNode } from "react";
export function AccountListRow({ children, current, selected, status, onInspect }: { children: ReactNode; current: boolean; selected: boolean; status: string; onInspect: () => void }) {
  return <article className={`accountRow uiAccountRow status-${status}${current ? " isCurrent" : ""}${selected ? " isSelected" : ""}`} aria-current={current ? "true" : undefined} onClick={onInspect}>{children}</article>;
}
