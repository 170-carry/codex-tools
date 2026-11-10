import type { ButtonHTMLAttributes } from "react";
export function Button({ busy = false, variant = "secondary", className = "", children, disabled, type = "button", ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { busy?: boolean; variant?: "primary" | "secondary" | "danger" | "quiet" }) {
  const style = { primary: "primary", secondary: "ghost", danger: "danger", quiet: "uiQuietButton" }[variant];
  return <button {...props} type={type} className={`uiButton ${style} ${className}`} disabled={disabled || busy} aria-busy={busy || undefined}>
    {busy && <span className="uiBusyIndicator" aria-hidden="true" />}{children}
  </button>;
}
