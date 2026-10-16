import { formatPlan, planTone } from "../../utils/usage";
import { accountIssueReason, accountStatus } from "../accounts/accountModel";
import { statusLabel } from "../accounts/accountCopy";
import { ActionIcon } from "./ClassicIcons";
import type { ClassicAccountsWorkspace } from "./useClassicAccounts";

export function ClassicAccountDetailHeader({
  workspace,
}: {
  workspace: ClassicAccountsWorkspace;
}) {
  const {
    copy,
    text,
    selectedRow,
    editingAliasId,
    aliasDraft,
    setAliasDraft,
    startAliasEdit,
    cancelAliasEdit,
    commitAliasEdit,
    renamingAccountId,
  } = workspace;
  if (!selectedRow) return null;

  const { account } = selectedRow;
  const status = accountStatus(account);
  const issueReason = accountIssueReason(account, text.issueFallbackReason);
  const plan = account.planType || account.usage?.planType;
  const editing = editingAliasId === account.id;
  const saving = renamingAccountId === account.accountKey;

  return (
    <header className="detailHeader">
      <div className="detailIdentity">
        <div className="detailTitleBlock">
          {editing ? (
            <div className="detailAliasEditor">
              <input
                value={aliasDraft}
                onChange={(event) => setAliasDraft(event.currentTarget.value)}
                disabled={saving}
                onKeyDown={(event) => {
                  if (event.key === "Escape") {
                    event.preventDefault();
                    cancelAliasEdit();
                  }
                  if (event.key === "Enter") {
                    event.preventDefault();
                    void commitAliasEdit(account);
                  }
                }}
                autoFocus
              />
              <button type="button" onClick={() => void commitAliasEdit(account)}>
                {text.save}
              </button>
              <button type="button" onClick={cancelAliasEdit}>
                {text.cancel}
              </button>
            </div>
          ) : (
            <h2 title={account.label}>{account.label}</h2>
          )}
          <div className="detailPlanStatus">
            <span className={`planChip tone-${planTone(plan)} isStatic`}>
              {formatPlan(plan, copy.accountCard.planLabels)}
            </span>
            <span
              className={`statusText status-${status}`}
              title={
                status === "issue"
                  ? (issueReason ?? text.issueFallbackReason)
                  : statusLabel(status, text)
              }
            >
              <span className="statusDot" />
              <span className="statusLabel">{statusLabel(status, text)}</span>
              {status === "issue" ? (
                <span className="statusReason">
                  {issueReason ?? text.issueFallbackReason}
                </span>
              ) : null}
            </span>
          </div>
        </div>
      </div>
      <button
        type="button"
        className="detailEditButton"
        onClick={() => startAliasEdit(account)}
        disabled={editing || saving}
      >
        <ActionIcon type="edit" />
        {text.edit}
      </button>
    </header>
  );
}
