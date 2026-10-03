import { EditorMultiSelect } from "../EditorMultiSelect";
import { SwitchField } from "../SwitchField";
import type { SettingsWorkspace } from "./useSettingsWorkspace";
export function SwitchingSettings({
  workspace,
}: {
  workspace: SettingsWorkspace;
}) {
  const {
    settings,
    installedEditorApps,
    hasOpencodeDesktopApp,
    savingSettings,
    onUpdateSettings,
    copy,
    pickingCodexLaunchPathKind,
    isWindows,
    pickCodexLaunchPath,
  } = workspace;
  return (
    <div className="settingsGroup">
      <SwitchField
        checked={settings.launchCodexAfterSwitch}
        onChange={(checked) =>
          onUpdateSettings({ launchCodexAfterSwitch: checked })
        }
        label={copy.settings.launchCodexAfterSwitch.label}
        checkedText={copy.settings.launchCodexAfterSwitch.checkedText}
        uncheckedText={copy.settings.launchCodexAfterSwitch.uncheckedText}
        disabled={savingSettings}
      />

      {isWindows ? (
        <SwitchField
          checked={settings.launchCodexAsAdmin}
          onChange={(checked) =>
            onUpdateSettings({ launchCodexAsAdmin: checked })
          }
          label={copy.settings.launchCodexAsAdmin.label}
          checkedText={copy.settings.launchCodexAsAdmin.checkedText}
          uncheckedText={copy.settings.launchCodexAsAdmin.uncheckedText}
          disabled={savingSettings || !settings.launchCodexAfterSwitch}
        />
      ) : null}

      <SwitchField
        checked={settings.smartSwitchIncludeApi}
        onChange={(checked) =>
          onUpdateSettings({ smartSwitchIncludeApi: checked })
        }
        label={copy.settings.smartSwitchIncludeApi.label}
        checkedText={copy.settings.smartSwitchIncludeApi.checkedText}
        uncheckedText={copy.settings.smartSwitchIncludeApi.uncheckedText}
        disabled={savingSettings}
      />

      <div className="settingRow">
        <div className="settingMeta">
          <strong>{copy.settings.codexLaunchPath.label}</strong>
        </div>
        <div className="settingFieldGroup">
          {settings.codexLaunchPath ? (
            <span className="settingPathValue">{settings.codexLaunchPath}</span>
          ) : null}
          <div className="settingActionGroup">
            {settings.codexLaunchPath ? (
              <button
                className="ghost settingPathClearButton"
                type="button"
                aria-label={copy.common.clear}
                disabled={savingSettings || pickingCodexLaunchPathKind !== null}
                onClick={() => onUpdateSettings({ codexLaunchPath: null })}
              >
                ×
              </button>
            ) : null}
            <button
              className="ghost"
              type="button"
              disabled={savingSettings || pickingCodexLaunchPathKind !== null}
              onClick={() => {
                void pickCodexLaunchPath("file");
              }}
            >
              {copy.addAccount.uploadChooseFiles}
            </button>
            <button
              className="ghost"
              type="button"
              disabled={savingSettings || pickingCodexLaunchPathKind !== null}
              onClick={() => {
                void pickCodexLaunchPath("directory");
              }}
            >
              {copy.addAccount.uploadChooseFolder}
            </button>
          </div>
        </div>
      </div>

      <SwitchField
        checked={settings.syncOpencodeOpenaiAuth}
        onChange={(checked) =>
          onUpdateSettings({ syncOpencodeOpenaiAuth: checked })
        }
        label={copy.settings.syncOpencode.label}
        checkedText={copy.settings.syncOpencode.checkedText}
        uncheckedText={copy.settings.syncOpencode.uncheckedText}
        disabled={savingSettings}
      />

      {settings.syncOpencodeOpenaiAuth && hasOpencodeDesktopApp ? (
        <SwitchField
          checked={settings.restartOpencodeDesktopOnSwitch}
          onChange={(checked) =>
            onUpdateSettings({ restartOpencodeDesktopOnSwitch: checked })
          }
          label={copy.settings.restartOpencodeDesktop.label}
          checkedText={copy.settings.restartOpencodeDesktop.checkedText}
          uncheckedText={copy.settings.restartOpencodeDesktop.uncheckedText}
          disabled={savingSettings}
          rowClassName="settingRowCompact settingRowNested"
        />
      ) : null}

      <SwitchField
        checked={settings.restartEditorsOnSwitch}
        onChange={(checked) => {
          if (
            checked &&
            settings.restartEditorTargets.length === 0 &&
            installedEditorApps.length > 0
          ) {
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
            <span className="settingValueMuted">
              {copy.settings.noSupportedEditors}
            </span>
          )}
        </div>
      ) : null}
    </div>
  );
}
