import { ClassicQuickActions } from "./ClassicQuickActions";
import { ClassicAccountDetailHeader } from "./ClassicAccountDetailHeader";
import { ClassicRecentSwitches } from "./ClassicRecentSwitches";
import { MembershipExpiry } from "../accounts/MembershipExpiry";
import { UsageFreshnessBadge } from "../accounts/UsageFreshnessBadge";
import { AccountOrderControls } from "../accounts/AccountOrderControls";
import { ResetCreditsSection } from "../accounts/ResetCreditsSection";
import { UsageMeter } from "./ClassicUsageMeter";
import { ClassicAccountTokenCard } from "./ClassicAccountTokenCard";
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
    switchRecords,
    expandedResetCreditsByAccount,
    toggleResetCredits,
    usageRefreshing,
    showInitialUsageRefresh,
    usageRefreshError,
    authBusy,
    onReauthorize,
    tokenUsageByAccount,
  } = workspace;

  return (
    <aside className="accountDetailPanel" aria-label={text.detailsTitle}>
      {selectedRow ? (
        <>
          <ClassicAccountDetailHeader workspace={workspace} />

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
              locale={locale}
            />
            <UsageMeter
              label={text.weekUsage}
              windowLabel="1w"
              window={selectedRow.account.usage?.oneWeek ?? null}
              text={text}
              locale={locale}
            />
          </section>

          <ClassicAccountTokenCard
            account={selectedRow.account}
            usage={tokenUsageByAccount.get(selectedRow.account.accountId)}
            locale={locale}
            emptyValue={text.emptyValue}
          />

          <section className="detailMetaGrid">
            <MembershipExpiry
              account={selectedRow.account}
              locale={locale}
              authBusy={authBusy}
              reauthorizeLabel={text.reauthorize}
              onReauthorize={onReauthorize}
            />
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

          <ClassicRecentSwitches
            records={switchRecords}
            locale={locale}
            text={text}
          />

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
