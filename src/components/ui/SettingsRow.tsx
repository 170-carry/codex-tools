import type { ReactNode } from "react";

export function SettingsRow({ label, children, layout = "inline", className = "" }: {
  label: ReactNode;
  children: ReactNode;
  layout?: "inline" | "stacked";
  className?: string;
}) {
  return <div className={`settingRow uiSettingsRow uiSettingsRow-${layout} ${className}`}>
    <div className="settingMeta"><strong>{label}</strong></div>
    <div className="uiSettingsControl">{children}</div>
  </div>;
}
