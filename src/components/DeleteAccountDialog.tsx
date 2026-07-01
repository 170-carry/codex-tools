import { createPortal } from "react-dom";
import { useI18n } from "../i18n/I18nProvider";
import type { AccountSummary } from "../types/app";

type DeleteAccountDialogProps = {
  account: AccountSummary | null;
  deleting: boolean;
  onCancel: () => void;
  onConfirm: () => void;
};

export function DeleteAccountDialog({
  account,
  deleting,
  onCancel,
  onConfirm,
}: DeleteAccountDialogProps) {
  const { copy } = useI18n();

  if (!account) {
    return null;
  }

  return createPortal(
    <div
      className="settingsOverlay"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) {
          onCancel();
        }
      }}
    >
      <div
        className="settingsDialog deleteAccountDialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="delete-account-title"
      >
        <div className="deleteAccountDialogIcon" aria-hidden="true">
          <svg
            className="iconGlyph"
            viewBox="0 0 24 24"
            focusable="false"
          >
            <path d="M3 6h18" />
            <path d="M8 6V4h8v2" />
            <path d="M19 6l-1 14H6L5 6" />
            <path d="M10 11v6" />
            <path d="M14 11v6" />
          </svg>
        </div>
        <div className="deleteAccountDialogBody">
          <h2 id="delete-account-title">{copy.accountDeleteDialog.title}</h2>
          <p>{copy.accountDeleteDialog.description(account.label)}</p>
          <strong title={account.label}>{account.label}</strong>
        </div>
        <div className="deleteAccountDialogActions">
          <button type="button" className="ghost" onClick={onCancel}>
            {copy.accountDeleteDialog.cancel}
          </button>
          <button
            type="button"
            className="danger"
            onClick={onConfirm}
            disabled={deleting}
          >
            {deleting
              ? copy.accountDeleteDialog.deleting
              : copy.accountDeleteDialog.confirm}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
