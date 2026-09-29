import { useState } from "react";
import { useI18n } from "../../i18n/I18nProvider";
import type { AccountSummary } from "../../types/app";
import type { ApiAccountEdit } from "../../hooks/useApiAccountEditor";

type Props = { account: AccountSummary; saving: boolean; error: string | null; onSave: (input: ApiAccountEdit) => Promise<void>; onClose: () => void };

export function EditApiAccountDialog({ account, saving, error, onSave, onClose }: Props) {
  const { copy } = useI18n();
  const text = copy.addAccount;
  const [draft, setDraft] = useState({ label: account.label, baseUrl: account.apiBaseUrl ?? "", modelName: account.modelName ?? "", apiKey: "" });
  const fields = [
    ["label", text.apiNameLabel], ["baseUrl", text.apiBaseUrlLabel],
    ["modelName", text.apiModelLabel], ["apiKey", text.apiKeyLabel],
  ] as const;
  return <div className="settingsOverlay" onClick={() => !saving && onClose()}>
    <section className="settingsDialog addAuthDialog" role="dialog" aria-modal="true" aria-label={text.apiEditTitle} onClick={(event) => event.stopPropagation()}>
      <div className="settingsHeader"><h2>{text.apiEditTitle}</h2><button type="button" disabled={saving} onClick={onClose}>{copy.common.close}</button></div>
      <form onSubmit={(event) => { event.preventDefault(); void onSave({ ...draft, apiKey: draft.apiKey.trim() || null }); }}>
        <p>{text.apiEditHint}</p>
        {fields.map(([key, label]) => <label className="addOauthField" key={key}>
          <span className="addOauthFieldLabel">{label}</span>
          <input className="addOauthInput" type={key === "apiKey" ? "password" : "text"} autoComplete="off" required={key !== "apiKey"} disabled={saving} value={draft[key]} onChange={(event) => setDraft({ ...draft, [key]: event.target.value })} />
        </label>)}
        {error && <p role="alert">{error}</p>}
        <button type="submit" disabled={saving}>{text.apiEditSave}</button>
      </form>
    </section>
  </div>;
}
