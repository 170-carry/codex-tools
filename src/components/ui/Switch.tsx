import type { MouseEventHandler } from "react";

export function Switch({ checked, onChange, label, disabled, className = "", onClick }: {
  checked: boolean; onChange: (checked: boolean) => void; label: string;
  disabled?: boolean; className?: string; onClick?: MouseEventHandler<HTMLLabelElement>;
}) {
  return <label className={`uiSwitch ${className}`} title={label} onClick={onClick}>
    <input type="checkbox" role="switch" aria-label={label} checked={checked} disabled={disabled}
      onChange={event => onChange(event.currentTarget.checked)} />
    <span className="uiSwitchTrack" aria-hidden="true"><span /></span>
  </label>;
}
