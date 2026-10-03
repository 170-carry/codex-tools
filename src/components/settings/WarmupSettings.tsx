import { SwitchField } from "../SwitchField";
import type { SettingsWorkspace } from "./useSettingsWorkspace";
export function WarmupSettings({
  workspace,
}: {
  workspace: SettingsWorkspace;
}) {
  const {
    settings,
    savingSettings,
    onUpdateSettings,
    copy,
    warmupAccounts,
    toggleWarmupAccount,
  } = workspace;
  return (
    <div className="settingsGroup">
      <SwitchField
        checked={settings.autoAccountWarmupEnabled}
        onChange={(checked) =>
          onUpdateSettings({ autoAccountWarmupEnabled: checked })
        }
        label={copy.settings.accountWarmup.label}
        checkedText={copy.settings.accountWarmup.checkedText}
        uncheckedText={copy.settings.accountWarmup.uncheckedText}
        disabled={savingSettings}
      />
      <div className="settingRow settingRowCompact settingRowNested warmupSettingsRow">
        <div className="settingMeta">
          <strong>{copy.settings.accountWarmup.accountsLabel}</strong>
          <span className="settingDescription">
            {copy.settings.accountWarmup.description}
          </span>
        </div>
        {warmupAccounts.length > 0 ? (
          <div className="warmupAccountChoices">
            {warmupAccounts.map((account) => (
              <label key={account.id} className="warmupAccountChoice">
                <input
                  type="checkbox"
                  checked={settings.autoAccountWarmupAccountIds.includes(
                    account.id,
                  )}
                  disabled={savingSettings}
                  onChange={(event) =>
                    toggleWarmupAccount(account.id, event.currentTarget.checked)
                  }
                />
                <span>{account.label}</span>
              </label>
            ))}
          </div>
        ) : (
          <span className="settingValueMuted">
            {copy.settings.accountWarmup.noAccounts}
          </span>
        )}
      </div>
    </div>
  );
}
