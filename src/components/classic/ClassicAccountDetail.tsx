import { ClassicQuickActions } from "./ClassicQuickActions";
import { formatPlan, planTone } from "../../utils/usage";
import { formatFullDate } from "../../utils/dateFormatting";
import { accountInitial } from "../accounts/accountPresentation";
import { accountIssueReason, accountStatus } from "../accounts/accountModel";
import { statusLabel } from "../accounts/accountCopy";
import { MembershipExpiry } from "../accounts/MembershipExpiry";
import { UsageFreshnessBadge } from "../accounts/UsageFreshnessBadge";
import { AccountOrderControls } from "../accounts/AccountOrderControls";
import { ResetCreditsSection } from "../accounts/ResetCreditsSection";
import { UsageMeter } from "./ClassicUsageMeter";
import { ActionIcon } from "./ClassicIcons";
import type { ClassicAccountsWorkspace } from "./useClassicAccounts";
export function ClassicAccountDetail({
  workspace,
}: {
  workspace: ClassicAccountsWorkspace;
}) {
  const {
    copy,
    locale,
    text,
    selectedRow,
    editingAliasId,
    aliasDraft,
    setAliasDraft,
    startAliasEdit,
    cancelAliasEdit,
    commitAliasEdit,
    switchRecords,
    expandedResetCreditsByAccount,
    toggleResetCredits,
    usageRefreshing,
    showInitialUsageRefresh,
    usageRefreshError,
    authBusy,
    renamingAccountId,
    onReauthorize,
  } = workspace;

  return (
    <aside className="accountDetailPanel" aria-label={text.detailsTitle}>
      {selectedRow ? (
        <>
          <header className="detailHeader">
            <div className="detailIdentity">
              <span
                className={`accountAvatar detailAvatar tone-${planTone(selectedRow.account.planType || selectedRow.account.usage?.planType)}`}
              >
                {accountInitial(selectedRow.account)}
              </span>
              <div className="detailTitleBlock">
                <span
                  className={`planChip tone-${planTone(selectedRow.account.planType || selectedRow.account.usage?.planType)} isStatic`}
                >
                  {formatPlan(
                    selectedRow.account.planType ||
                      selectedRow.account.usage?.planType,
                    copy.accountCard.planLabels,
                  )}
                </span>
                {editingAliasId === selectedRow.account.id ? (
                  <div className="detailAliasEditor">
                    <input
                      value={aliasDraft}
                      onChange={(event) =>
                        setAliasDraft(event.currentTarget.value)
                      }
                      disabled={
                        renamingAccountId === selectedRow.account.accountKey
                      }
                      onKeyDown={(event) => {
                        if (event.key === "Escape") {
                          event.preventDefault();
                          cancelAliasEdit();
                        }
                        if (event.key === "Enter") {
                          event.preventDefault();
                          void commitAliasEdit(selectedRow.account);
                        }
                      }}
                      autoFocus
                    />
                    <button
                      type="button"
                      onClick={() => void commitAliasEdit(selectedRow.account)}
                    >
                      {text.save}
                    </button>
                    <button type="button" onClick={cancelAliasEdit}>
                      {text.cancel}
                    </button>
                  </div>
                ) : (
                  <h2>{selectedRow.account.label}</h2>
                )}
                <span
                  className={`statusText status-${accountStatus(selectedRow.account)}`}
                  title={
                    accountStatus(selectedRow.account) === "issue"
                      ? (accountIssueReason(
                          selectedRow.account,
                          text.issueFallbackReason,
                        ) ?? text.issueFallbackReason)
                      : statusLabel(accountStatus(selectedRow.account), text)
                  }
                >
                  <span className="statusDot" />
                  <span className="statusLabel">
                    {statusLabel(accountStatus(selectedRow.account), text)}
                  </span>
                  {accountStatus(selectedRow.account) === "issue" ? (
                    <span className="statusReason">
                      {accountIssueReason(
                        selectedRow.account,
                        text.issueFallbackReason,
                      ) ?? text.issueFallbackReason}
                    </span>
                  ) : null}
                </span>
              </div>
            </div>
            <button
              type="button"
              className="detailEditButton"
              onClick={() => startAliasEdit(selectedRow.account)}
              disabled={
                editingAliasId === selectedRow.account.id ||
                renamingAccountId === selectedRow.account.accountKey
              }
            >
              <ActionIcon type="edit" />
              {text.edit}
            </button>
          </header>

          <section className="detailCard">
            <div className="detailCardTitle">
              <h3>{text.usageOverview}</h3>
              <UsageFreshnessBadge
                account={selectedRow.account}
                refreshing={usageRefreshing}
                showInitialRefresh={showInitialUsageRefresh}
                refreshError={usageRefreshError}
                locale={locale}
                copy={copy.accountsGrid}
              />
            </div>
            <UsageMeter
              label={text.fiveHourUsage}
              windowLabel="5h"
              window={selectedRow.account.usage?.fiveHour ?? null}
              text={text}
            />
            <UsageMeter
              label={text.weekUsage}
              windowLabel="1w"
              window={selectedRow.account.usage?.oneWeek ?? null}
              text={text}
            />
          </section>

          <section className="detailMetaGrid">
            <MembershipExpiry
              account={selectedRow.account}
              locale={locale}
              authBusy={authBusy}
              reauthorizeLabel={text.reauthorize}
              onReauthorize={onReauthorize}
            />
            <div>
              <span>{text.planType}</span>
              <strong>
                {formatPlan(
                  selectedRow.account.planType ||
                    selectedRow.account.usage?.planType,
                  copy.accountCard.planLabels,
                )}
              </strong>
            </div>
          </section>

          <AccountOrderControls account={selectedRow.account} actions={workspace.actions} />
          <ResetCreditsSection
            key={selectedRow.account.id}
            busy={workspace.actions.authBusy}
            onUseCredit={workspace.actions.onUseResetCredit}
            account={selectedRow.account}
            expanded={Boolean(
              expandedResetCreditsByAccount[selectedRow.account.id],
            )}
            locale={locale}
            text={text}
            onToggle={() => toggleResetCredits(selectedRow.account.id)}
          />

          <section className="detailCard recentSwitchCard">
            <div className="detailSectionTitle">
              <h3>{text.recentSwitches}</h3>
            </div>
            {switchRecords.length > 0 ? (
              <ul className="recentSwitchList">
                {switchRecords.map((record) => (
                  <li key={record.id}>
                    <span className="statusDot status-using" />
                    <span>{text.switchRecordAction}</span>
                    <strong>
                      {formatFullDate(
                        record.timestamp,
                        locale,
                        text.emptyValue,
                      )}
                    </strong>
                    <em>
                      {record.source === text.emptyValue
                        ? record.target
                        : `${text.fromPrefix} ${record.source}`}
                    </em>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="recentSwitchEmpty">{text.noSwitchRecords}</p>
            )}
          </section>

          <ClassicQuickActions workspace={workspace} />
        </>
      ) : (
        <div className="detailEmpty">
          <h3>{copy.accountsGrid.emptyTitle}</h3>
          <p>{copy.accountsGrid.emptyDescription}</p>
        </div>
      )}
    </aside>
  );
}
