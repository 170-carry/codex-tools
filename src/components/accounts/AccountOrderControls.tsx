import { useI18n } from "../../i18n/I18nProvider";
import { sortAccountsForDisplay } from "../../utils/accountDisplayOrder";
import type { AccountSummary } from "../../types/app";
import type { AccountsGridProps } from "./types";

export function AccountOrderControls({
  account,
  actions,
}: {
  account: AccountSummary;
  actions: AccountsGridProps;
}) {
  const { copy } = useI18n();
  if (!actions.onMoveAccount) return null;
  const keys = [
    ...new Set(
      sortAccountsForDisplay(actions.accounts, actions.accountOrder).map(
        (item) => item.accountKey,
      ),
    ),
  ];
  const index = keys.indexOf(account.accountKey);
  const busy = actions.authBusy || actions.orderingAccounts;
  return (
    <section className="detailCard">
      <h3>{copy.apiProxy.accountOrderLabel}</h3>
      <div className="settingActionGroup">
        <button
          type="button"
          className="ghost"
          disabled={busy || index <= 0}
          onClick={() => actions.onMoveAccount?.(account, -1)}
        >
          {copy.apiProxy.moveUp}
        </button>
        <span aria-live="polite">{index + 1} / {keys.length}</span>
        <button
          type="button"
          className="ghost"
          disabled={busy || index < 0 || index === keys.length - 1}
          onClick={() => actions.onMoveAccount?.(account, 1)}
        >
          {copy.apiProxy.moveDown}
        </button>
      </div>
    </section>
  );
}
