import { SwitchField } from "../SwitchField";
import type { SettingsWorkspace } from "./useSettingsWorkspace";

function formatMinute(minute: number): string {
  return `${Math.floor(minute / 60).toString().padStart(2, "0")}:${(minute % 60).toString().padStart(2, "0")}`;
}
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
      <SwitchField
        checked={settings.autoAccountWarmupScheduleEnabled}
        onChange={(checked) => onUpdateSettings({ autoAccountWarmupScheduleEnabled: checked })}
        label={copy.settings.accountWarmup.scheduleLabel}
        checkedText={copy.settings.accountWarmup.checkedText}
        uncheckedText={copy.settings.accountWarmup.uncheckedText}
        disabled={savingSettings || !settings.autoAccountWarmupEnabled}
      />
      {settings.autoAccountWarmupScheduleEnabled ? (
        <div className="settingRow settingRowNested">
          <div className="settingMeta">
            <span className="settingDescription">{copy.settings.accountWarmup.scheduleDescription}</span>
          </div>
          <div className="settingFieldGroup">
            {(["start", "end"] as const).map((boundary) => {
              const field = boundary === "start" ? "autoAccountWarmupStartMinute" : "autoAccountWarmupEndMinute";
              return (
                <label key={boundary}>
                  <span>{boundary === "start" ? copy.settings.accountWarmup.startTime : copy.settings.accountWarmup.endTime}</span>
                  <input
                    type="time"
                    value={formatMinute(settings[field])}
                    disabled={savingSettings || !settings.autoAccountWarmupEnabled}
                    onChange={(event) => {
                      if (!/^\d{2}:\d{2}$/.test(event.currentTarget.value)) return;
                      const [hours, minutes] = event.currentTarget.value.split(":").map(Number);
                      onUpdateSettings({ [field]: hours * 60 + minutes });
                    }}
                  />
                </label>
              );
            })}
          </div>
        </div>
      ) : null}
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
