import { Switch } from "./ui/Switch";
import { SettingsRow } from "./ui/SettingsRow";
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
  disabled = false,
  rowClassName,
}: SwitchFieldProps) {
  return (
    <SettingsRow label={label} className={rowClassName}>
      <Switch checked={checked} onChange={onChange} label={label} disabled={disabled} />
    </SettingsRow>
  );
}
