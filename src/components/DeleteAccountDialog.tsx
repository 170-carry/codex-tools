import { useI18n } from "../i18n/I18nProvider";
import type { AccountSummary } from "../types/app";
import { UtilityDialog } from "./workspace/UtilityDialog";

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
  if (!account) return null;
  return (
    <UtilityDialog
      role="alertdialog"
      className="deleteAccountDialog"
      title={copy.accountDeleteDialog.title}
      description={copy.accountDeleteDialog.description(account.label)}
      closeLabel={copy.common.close}
      onClose={onCancel}
      dismissible={!deleting}
      showClose={false}
      actions={
        <>
          <button
            type="button"
            className="ghost"
            onClick={onCancel}
            disabled={deleting}
          >
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
        </>
      }
    />
  );
}
