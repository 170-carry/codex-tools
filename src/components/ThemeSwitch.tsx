import { Switch } from "./ui/Switch";
import type { ThemeMode } from "../types/app";
import { useI18n } from "../i18n/I18nProvider";

type ThemeSwitchProps = {
  themeMode: ThemeMode;
  onToggle: () => void;
};

export function ThemeSwitch({ themeMode, onToggle }: ThemeSwitchProps) {
  const { copy } = useI18n();
  const isDark = themeMode === "dark";

  return (
    <Switch checked={isDark} onChange={onToggle} label={copy.settings.theme.switchAriaLabel} />
  );
}
