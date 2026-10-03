import { useState } from "react";
import { useI18n } from "../../i18n/I18nProvider";
import type { AccountSummary } from "../../types/app";
import type { AccountsGridProps } from "./types";

export function AccountProxyToggle({
  account,
  disabled,
  onToggle,
}: {
  account: AccountSummary;
  disabled: boolean;
  onToggle: AccountsGridProps["onToggleApiProxy"];
}) {
  const { copy } = useI18n();
  const [saving, setSaving] = useState(false);
  const label = `${account.label || account.email || account.id} · ${copy.accountCard.apiProxyToggle}`;
  const update = async (enabled: boolean) => {
    if (disabled || saving) return;
    setSaving(true);
    try {
      await onToggle(account, enabled);
    } finally {
      setSaving(false);
    }
  };

  return (
    <label
      className={`themeSwitch accountProxyToggle${disabled || saving ? " isDisabled" : ""}`}
      title={label}
      aria-busy={saving}
    >
      <input
        type="checkbox"
        role="switch"
        aria-label={label}
        checked={account.apiProxyEnabled}
        disabled={disabled || saving}
        onChange={(event) => void update(event.currentTarget.checked)}
      />
      <span className="themeSwitchTrack" aria-hidden="true">
        <span className="themeSwitchThumb" />
      </span>
    </label>
  );
}
