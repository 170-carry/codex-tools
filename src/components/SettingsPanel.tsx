import { useI18n } from "../i18n/I18nProvider";
import { EditorMultiSelect } from "./EditorMultiSelect";
import { ThemeSwitch } from "./ThemeSwitch";
import { SwitchField } from "./SwitchField";
import type {
  AppSettings,
  InstalledEditorApp,
  ThemeMode,
  UpdateSettingsOptions,
} from "../types/app";

type SettingsPanelProps = {
  themeMode: ThemeMode;
  onToggleTheme: () => void;
  checkingUpdate: boolean;
  onCheckUpdate: () => void;
  settings: AppSettings;
  installedEditorApps: InstalledEditorApp[];
  savingSettings: boolean;
  onUpdateSettings: (patch: Partial<AppSettings>, options?: UpdateSettingsOptions) => void;
};

export function SettingsPanel({
  themeMode,
  onToggleTheme,
  checkingUpdate,
  onCheckUpdate,
  settings,
  installedEditorApps,
  savingSettings,
  onUpdateSettings,
}: SettingsPanelProps) {
  const { copy, locale, localeOptions, setLocale } = useI18n();
  const showThemeSetting = false;
  const languageLabel = copy.topBar.languagePicker;
  const languageOptions = localeOptions.map((item) => ({
    id: item.code,
    label: item.nativeLabel,
  }));

  return (
    <section className="settingsPage" aria-label={copy.settings.title}>
      <div className="settingsShell">
        <div className="settingsHeader">
          <div>
            <h2>{copy.settings.title}</h2>
          </div>
        </div>

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

          {showThemeSetting ? (
            <div className="settingRow">
              <div className="settingMeta">
                <strong>{copy.settings.theme.label}</strong>
              </div>
              <ThemeSwitch themeMode={themeMode} onToggle={onToggleTheme} />
            </div>
          ) : null}

          <div className="settingRow">
            <div className="settingMeta">
              <strong>{copy.settings.trayUsageDisplay.label}</strong>
            </div>
            <div className="modeGroup" role="radiogroup" aria-label={copy.settings.trayUsageDisplay.groupAriaLabel}>
              <button
                className={settings.trayUsageDisplayMode === "remaining" ? "primary" : "ghost"}
                disabled={savingSettings}
                onClick={() => onUpdateSettings({ trayUsageDisplayMode: "remaining" })}
                aria-pressed={settings.trayUsageDisplayMode === "remaining"}
              >
                {copy.settings.trayUsageDisplay.remaining}
              </button>
              <button
                className={settings.trayUsageDisplayMode === "used" ? "primary" : "ghost"}
                disabled={savingSettings}
                onClick={() => onUpdateSettings({ trayUsageDisplayMode: "used" })}
                aria-pressed={settings.trayUsageDisplayMode === "used"}
              >
                {copy.settings.trayUsageDisplay.used}
              </button>
              <button
                className={settings.trayUsageDisplayMode === "hidden" ? "primary" : "ghost"}
                disabled={savingSettings}
                onClick={() => onUpdateSettings({ trayUsageDisplayMode: "hidden" })}
                aria-pressed={settings.trayUsageDisplayMode === "hidden"}
              >
                {copy.settings.trayUsageDisplay.hidden}
              </button>
            </div>
          </div>
        </div>

        <div className="settingsGroup">
          <SwitchField
            checked={settings.launchAtStartup}
            onChange={(checked) => onUpdateSettings({ launchAtStartup: checked })}
            label={copy.settings.launchAtStartup.label}
            checkedText={copy.settings.launchAtStartup.checkedText}
            uncheckedText={copy.settings.launchAtStartup.uncheckedText}
            disabled={savingSettings}
          />

          <SwitchField
            checked={settings.launchCodexAfterSwitch}
            onChange={(checked) => onUpdateSettings({ launchCodexAfterSwitch: checked })}
            label={copy.settings.launchCodexAfterSwitch.label}
            checkedText={copy.settings.launchCodexAfterSwitch.checkedText}
            uncheckedText={copy.settings.launchCodexAfterSwitch.uncheckedText}
            disabled={savingSettings}
          />

          <SwitchField
            checked={settings.syncOpencodeOpenaiAuth}
            onChange={(checked) => onUpdateSettings({ syncOpencodeOpenaiAuth: checked })}
            label={copy.settings.syncOpencode.label}
            checkedText={copy.settings.syncOpencode.checkedText}
            uncheckedText={copy.settings.syncOpencode.uncheckedText}
            disabled={savingSettings}
          />

          <SwitchField
            checked={settings.restartEditorsOnSwitch}
            onChange={(checked) => {
              if (checked && settings.restartEditorTargets.length === 0 && installedEditorApps.length > 0) {
                onUpdateSettings({
                  restartEditorsOnSwitch: true,
                  restartEditorTargets: [installedEditorApps[0].id],
                });
                return;
              }
              onUpdateSettings({ restartEditorsOnSwitch: checked });
            }}
            label={copy.settings.restartEditorsOnSwitch.label}
            checkedText={copy.settings.restartEditorsOnSwitch.checkedText}
            uncheckedText={copy.settings.restartEditorsOnSwitch.uncheckedText}
            disabled={savingSettings}
          />

          {settings.restartEditorsOnSwitch ? (
            <div className="settingRow settingRowCompact settingRowNested">
              <div className="settingMeta">
                <strong>{copy.settings.restartEditorTargets.label}</strong>
              </div>
              {installedEditorApps.length > 0 ? (
                <EditorMultiSelect
                  options={installedEditorApps}
                  value={settings.restartEditorTargets[0] ?? null}
                  onChange={(selected) =>
                    onUpdateSettings(
                      { restartEditorTargets: [selected] },
                      { silent: true, keepInteractive: true },
                    )
                  }
                />
              ) : (
                <span className="settingValueMuted">{copy.settings.noSupportedEditors}</span>
              )}
            </div>
          ) : null}
        </div>

        <div className="settingsGroup settingsGroupAction">
          <div className="settingRow">
            <div className="settingMeta">
              <strong>{copy.topBar.checkUpdate}</strong>
            </div>
            <button className="primary" onClick={onCheckUpdate} disabled={checkingUpdate}>
              {checkingUpdate ? copy.topBar.checkingUpdate : copy.topBar.checkUpdate}
            </button>
          </div>
        </div>
      </div>
    </section>
  );
}
