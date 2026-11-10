import { PlanStamp } from "./PlanStamp";
import { Switch } from "../ui/Switch";
import { AccountListRow } from "../ui/AccountListRow";
import type { AccountRow } from "../accounts/types";
import { formatPlan, planTone } from "../../utils/usage";
import { displayAccountAddress } from "../accounts/accountPresentation";
import { accountIssueReason, accountStatus } from "../accounts/accountModel";
import { statusLabel } from "../accounts/accountCopy";
import { UsageFreshnessBadge } from "../accounts/UsageFreshnessBadge";
import { AccountActionMenu } from "../accounts/AccountActionMenu";
import { AccountRowMetrics } from "./AccountRowMetrics";
import { UsageMeter } from "./ClassicUsageMeter";
import type { ClassicAccountsWorkspace } from "./useClassicAccounts";
export function ClassicAccountRow({
  workspace,
  row,
}: {
  workspace: ClassicAccountsWorkspace;
  row: AccountRow;
}) {
  const {
    copy,
    locale,
    text,
    selectedRow,
    selectAccount,
    selectVariant,
    handleSwitch,
    usageRefreshing,
    showInitialUsageRefresh,
    usageRefreshError,
    authBusy,
    switchingId,
    onToggleApiProxy,
  } = workspace;
  const actions = workspace.actions;
  const account = row.account;
  const status = accountStatus(account);
  const isSelected = workspace.inspectorOpen && selectedRow?.account.id === account.id;
  const isSwitching = switchingId === account.id;
  const switchDisabled = authBusy || account.isCurrent;
  const accountAddress = displayAccountAddress(account, text.emptyValue);
  const issueReason = accountIssueReason(account, text.issueFallbackReason);
  return (
    <AccountListRow current={account.isCurrent} selected={isSelected} status={status} onInspect={() => selectAccount(account.id)}>
      <div className="accountIdentityCell">

        <span className="accountIdentityText">
          <span className="accountTitleLine">
            <span className="accountPlanBadges">
            {row.variants.map((variant) => {
              const plan = formatPlan(
                variant.planType || variant.usage?.planType,
                copy.accountCard.planLabels,
              );
              const isVariantSelected = variant.id === account.id;

              return (
                <button
                  key={variant.id}
                  type="button"
                  className={`planChip accountPlanStamp tone-${planTone(variant.planType || variant.usage?.planType)}${
                    isVariantSelected ? " isSelected" : ""
                  }`}
                  onClick={(event) => {
                    event.stopPropagation();
                    selectVariant(row.id, variant);
                  }}
                  aria-label={plan}
                  title={plan}
                  aria-pressed={isVariantSelected}
                >
                  <PlanStamp label={plan} />
                </button>
              );
            })}
            </span>
            <button
              type="button"
              className="accountNameButton"
              title={accountAddress}
              onClick={(event) => {
                event.stopPropagation();
                selectAccount(account.id);
              }}
            >
              {account.label || accountAddress}
            </button>
          </span>
          <span className="accountStateLine">
            <AccountRowMetrics account={account} usage={workspace.tokenUsageByAccount.get(account.accountId)} locale={locale} text={text} />
              <span className={`statusText status-${status}`} role="img"
                aria-label={statusLabel(status, text)}
                title={status === "issue" ? (issueReason ?? text.issueFallbackReason) : statusLabel(status, text)}>
                <span className="statusDot" />
              </span>
            <UsageFreshnessBadge
              account={account}
              refreshing={usageRefreshing}
              showInitialRefresh={showInitialUsageRefresh}
              refreshError={usageRefreshError}
              locale={locale}
              copy={copy.accountsGrid}
            />
          </span>
        </span>
      </div>
      <UsageMeter
        className="accountUsageFive"
        label={text.fiveHourUsage}
        windowLabel="5h"
        window={account.usage?.fiveHour ?? null}
        text={text}
        locale={locale}
      />
      <UsageMeter
        className="accountUsageWeek"
        label={text.weekUsage}
        windowLabel="1w"
        window={account.usage?.oneWeek ?? null}
        text={text}
        locale={locale}
      />
      <div className="accountProxyControl" onClick={event => event.stopPropagation()}>
        <span>API</span>
        <Switch checked={account.apiProxyEnabled} disabled={authBusy}
          label={copy.accountCard.apiProxyToggle}
          onChange={checked => { void onToggleApiProxy(account, checked); }} />
      </div>
      <div className="rowActions" onClick={(event) => event.stopPropagation()}>
        <button
          type="button"
          className="rowSwitchButton"
          disabled={switchDisabled}
          onClick={(event) => {
            event.stopPropagation();
            void handleSwitch(account);
          }}
        >
          {isSwitching
            ? copy.accountCard.launching
            : account.isCurrent
              ? text.statusUsing
              : text.switchAccount}
        </button>
        <AccountActionMenu
          account={account}
          actions={actions}
          text={text}
          onInspect={() => selectAccount(account.id)}
        />
      </div>
    </AccountListRow>
  );
}
