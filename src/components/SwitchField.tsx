type SwitchFieldProps = {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  checkedText: string;
  uncheckedText: string;
  disabled?: boolean;
  rowClassName?: string;
};

export function SwitchField({
  checked,
  onChange,
  label,
  checkedText,
  uncheckedText,
  disabled = false,
  rowClassName,
}: SwitchFieldProps) {
  return (
    <div className={["settingRow", rowClassName].filter(Boolean).join(" ")}>
      <div className="settingMeta">
        <strong>{label}</strong>
      </div>
      <label className="themeSwitch" aria-label={label}>
        <input
          type="checkbox"
          checked={checked}
          disabled={disabled}
          onChange={(event) => onChange(event.target.checked)}
        />
        <span className="themeSwitchTrack" aria-hidden="true">
          <span className="themeSwitchThumb" />
        </span>
        <span className="themeSwitchText">{checked ? checkedText : uncheckedText}</span>
      </label>
    </div>
  );
}
