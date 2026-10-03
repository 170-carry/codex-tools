import { SwitchField } from "../SwitchField";
import { EditorMultiSelect } from "../EditorMultiSelect";
import { ThemeSwitch } from "../ThemeSwitch";
import type { SettingsWorkspace } from "./useSettingsWorkspace";
export function GeneralSettings({
  workspace,
}: {
  workspace: SettingsWorkspace;
}) {
  const {
    settings,
    savingSettings,
    onUpdateSettings,
    themeMode,
    onToggleTheme,
    copy,
    locale,
    setLocale,
    languageLabel,
    languageOptions,
  } = workspace;
  return (
    <div className="settingsGroup">
      <div className="settingRow">
        <div className="settingMeta">
          <strong>{languageLabel}</strong>
        </div>
        <EditorMultiSelect
          options={languageOptions}
          value={locale}
          className="languagePicker"
          ariaLabel={languageLabel}
          placeholder={languageLabel}
          onChange={setLocale}
        />
      </div>
      <div className="settingRow">
        <div className="settingMeta">
          <strong>{copy.settings.theme.label}</strong>
        </div>
        <ThemeSwitch themeMode={themeMode} onToggle={onToggleTheme} />
      </div>
      <SwitchField
        checked={settings.launchAtStartup}
        onChange={(checked) => onUpdateSettings({ launchAtStartup: checked })}
        label={copy.settings.launchAtStartup.label}
        checkedText={copy.settings.launchAtStartup.checkedText}
        uncheckedText={copy.settings.launchAtStartup.uncheckedText}
        disabled={savingSettings}
      />
    </div>
  );
}
