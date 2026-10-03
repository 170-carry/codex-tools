import { useId, useState } from "react";
import { useI18n } from "../../i18n/I18nProvider";
import type { AccountSummary } from "../../types/app";
import type { ApiAccountEdit } from "../../hooks/useApiAccountEditor";
import { UtilityDialog } from "../workspace/UtilityDialog";

type Props = {
  account: AccountSummary;
  saving: boolean;
  error: string | null;
  onSave: (input: ApiAccountEdit) => Promise<void>;
  onClose: () => void;
};

export function EditApiAccountDialog({ account, saving, error, onSave, onClose }: Props) {
  const { copy } = useI18n();
  const text = copy.addAccount;
  const formId = useId();
  const [draft, setDraft] = useState({
    label: account.label,
    baseUrl: account.apiBaseUrl ?? "",
    modelName: account.modelName ?? "",
    apiKey: "",
  });
  const fields = [
    ["label", text.apiNameLabel],
    ["baseUrl", text.apiBaseUrlLabel],
    ["modelName", text.apiModelLabel],
    ["apiKey", text.apiKeyLabel],
  ] as const;
  return (
    <UtilityDialog
      title={text.apiEditTitle}
      description={text.apiEditHint}
      className="apiAccountEditDialog"
      closeLabel={copy.common.close}
      dismissible={!saving}
      onClose={onClose}
      actions={
        <>
          <button type="button" className="ghost" disabled={saving} onClick={onClose}>
            {copy.accountDeleteDialog.cancel}
          </button>
          <button type="submit" form={formId} className="primary" disabled={saving}>
            {text.apiEditSave}
          </button>
        </>
      }
    >
      <form id={formId} onSubmit={(event) => {
        event.preventDefault();
        void onSave({ ...draft, apiKey: draft.apiKey.trim() || null });
      }}>
        {fields.map(([key, label]) => (
          <label className="addOauthField" key={key}>
            <span className="addOauthFieldLabel">{label}</span>
            <input
              className="addOauthInput"
              type={key === "apiKey" ? "password" : "text"}
              autoComplete="off"
              required={key !== "apiKey"}
              disabled={saving}
              value={draft[key]}
              onChange={(event) => setDraft({ ...draft, [key]: event.target.value })}
            />
          </label>
        ))}
        {error && <p role="alert">{error}</p>}
      </form>
    </UtilityDialog>
  );
}
