import type { AccountRow } from "../accounts/types";
import { formatPlan, planTone } from "../../utils/usage";
import { formatFullDate } from "../../utils/dateFormatting";
import {
  accountInitial,
  displayAccountAddress,
  formatResetValue,
} from "../accounts/accountPresentation";
import { accountIssueReason, accountStatus } from "../accounts/accountModel";
import { statusLabel } from "../accounts/accountCopy";
import { UsageFreshnessBadge } from "../accounts/UsageFreshnessBadge";
import { AccountActionMenu } from "../accounts/AccountActionMenu";
import { AccountResetCredits } from "../accounts/AccountResetCredits";
import { UsageMeter } from "./ClassicUsageMeter";
import { copyAccountText } from "./copyAccountText";
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
  const normalizedPlan = account.planType || account.usage?.planType;
  const isSelected = selectedRow?.account.id === account.id;
  const isSwitching = switchingId === account.id;
  const switchDisabled = authBusy || account.isCurrent;
  const accountAddress = displayAccountAddress(account, text.emptyValue);
  const issueReason = accountIssueReason(account, text.issueFallbackReason);
  return (
    <article
      key={row.id}
      className={`accountRow status-${status}${isSelected ? " isSelected" : ""}${account.isCurrent ? " isCurrent" : ""}`}
      aria-current={account.isCurrent ? "true" : undefined}
      role="button"
      tabIndex={0}
      onClick={() => selectAccount(account.id)}
      onKeyDown={(event) => {
        if (
          event.target === event.currentTarget &&
          (event.key === "Enter" || event.key === " ")
        ) {
          event.preventDefault();
          selectAccount(account.id);
        }
      }}
    >
      <div className="accountIdentityCell">
        <span className={`accountAvatar tone-${planTone(normalizedPlan)}`}>
          {accountInitial(account)}
        </span>
        <span className="accountIdentityText">
          <span className="accountTitleLine">
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
                  className={`planChip tone-${planTone(variant.planType || variant.usage?.planType)}${
                    isVariantSelected ? " isSelected" : ""
                  }`}
                  onClick={(event) => {
                    event.stopPropagation();
                    selectVariant(row.id, variant);
                  }}
                  aria-pressed={isVariantSelected}
                >
                  {plan}
                </button>
              );
            })}
            <button
              type="button"
              className="accountNameButton"
              title={accountAddress}
              onClick={(event) => {
                event.stopPropagation();
                copyAccountText(accountAddress);
              }}
            >
              {account.label || accountAddress}
            </button>
          </span>
          <span className="accountStateLine">
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
            <AccountResetCredits account={account} text={text} />
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
      />
      <UsageMeter
        className="accountUsageWeek"
        label={text.weekUsage}
        windowLabel="1w"
        window={account.usage?.oneWeek ?? null}
        text={text}
      />
      <div className="resetCell">
        <span
          title={formatFullDate(
            account.usage?.fiveHour?.resetAt,
            locale,
            text.emptyValue,
          )}
        >
          {formatResetValue(
            account.usage?.fiveHour?.resetAt,
            locale,
            text.emptyValue,
          )}
        </span>
        <strong
          title={formatFullDate(
            account.usage?.oneWeek?.resetAt,
            locale,
            text.emptyValue,
          )}
        >
          {formatResetValue(
            account.usage?.oneWeek?.resetAt,
            locale,
            text.emptyValue,
          )}
        </strong>
      </div>
      <label className="rowToggle" onClick={(event) => event.stopPropagation()}>
        <input
          type="checkbox"
          checked={account.apiProxyEnabled}
          disabled={authBusy}
          onChange={(event) => {
            void onToggleApiProxy(account, event.currentTarget.checked);
          }}
          aria-label={copy.accountCard.apiProxyToggle}
        />
        <span />
      </label>
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
    </article>
  );
}
