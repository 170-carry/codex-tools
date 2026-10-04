import { ActionIcon } from "./ClassicIcons";
import type { ClassicAccountsWorkspace } from "./useClassicAccounts";
export function ClassicQuickActions({
  workspace,
}: {
  workspace: ClassicAccountsWorkspace;
}) {
  const {
    copy,
    text,
    selectedRow,
    handleSwitch,
    exportingAccounts,
    authBusy,
    warmingAccountId,
    onExport,
    onReauthorize,
    onWarmup,
    onDelete,
  } = workspace;
  if (!selectedRow) return null;
  return (
    <section className="detailCard quickActionCard">
      <h3>{text.quickActions}</h3>
      <div className="quickActionGrid">
        {selectedRow.account.sourceKind === "relay" ? (
          <button
            type="button"
            disabled={authBusy}
            onClick={() =>
              workspace.actions.onEditApiAccount(selectedRow.account)
            }
          >
            <ActionIcon type="edit" />
            <span>{copy.addAccount.apiEditTitle}</span>
          </button>
        ) : null}
        <button
          type="button"
          onClick={() => onReauthorize(selectedRow.account)}
        >
          <ActionIcon type="login" />
          <span>{text.reauthorize}</span>
        </button>
        {selectedRow.account.sourceKind !== "relay" ? (
          <button
            type="button"
            onClick={() => void onWarmup(selectedRow.account)}
            disabled={warmingAccountId !== null || authBusy}
          >
            <ActionIcon type="warmup" />
            <span>
              {warmingAccountId === selectedRow.account.id
                ? text.warming
                : text.warmup}
            </span>
          </button>
        ) : null}
        <button
          type="button"
          onClick={() => {
            void handleSwitch(selectedRow.account);
          }}
          disabled={authBusy || selectedRow.account.isCurrent}
        >
          <ActionIcon type="switch" />
          <span>{text.switchAccount}</span>
        </button>
        <button
          type="button"
          onClick={() => onExport(selectedRow.account)}
          disabled={exportingAccounts}
        >
          <ActionIcon type="export" />
          <span>{text.exportAccount}</span>
        </button>
        <button
          type="button"
          className="dangerAction"
          onClick={() => onDelete(selectedRow.account)}
        >
          <ActionIcon type="delete" />
          <span>{text.deleteAccount}</span>
        </button>
      </div>
    </section>
  );
}
