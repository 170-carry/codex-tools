import { useI18n } from "../../i18n/I18nProvider";
import type { AccountSummary } from "../../types/app";
import type { AccountRow, AccountsGridProps, UiCopy } from "./types";
import { accountIssueReason, accountStatus } from "./accountModel";
import { displayAccountAddress } from "./accountPresentation";
import { statusLabel } from "./accountCopy";
import { AccountPlanTags } from "./AccountPlanTags";
import { AccountResetCredits } from "./AccountResetCredits";
import { QuotaMeter } from "./QuotaMeter";
import { UsageFreshnessBadge } from "./UsageFreshnessBadge";
import { AccountActionMenu } from "./AccountActionMenu";
import { AccountProxyToggle } from "./AccountProxyToggle";

export function AccountListRow({
  row,
  actions,
  text,
  onInspect,
  onVariant,
  onSwitch,
}: {
  row: AccountRow;
  actions: AccountsGridProps;
  text: UiCopy;
  onInspect: (id: string) => void;
  onVariant: (groupId: string, accountId: string) => void;
  onSwitch: (account: AccountSummary) => Promise<void>;
}) {
  const { copy, locale } = useI18n();
  const { account } = row;
  const status = accountStatus(account);
  const address = displayAccountAddress(account, text.emptyValue);
  return (
    <tr
      className={`accountRow status-${status}${account.isCurrent ? " isCurrent" : ""}`}
      aria-current={account.isCurrent ? "true" : undefined}
    >
      <td>
        <div className="accountIdentityCell">
          <div className="accountIdentityText">
            <div className="accountTitleLine">
              <button
                type="button"
                className="accountNameButton"
                title={address}
                onClick={() => onInspect(account.id)}
              >
                {account.label || address}
              </button>
              {account.isCurrent ? (
                <span className="visuallyHidden">{text.statusUsing}</span>
              ) : null}
              <AccountPlanTags
                variants={row.variants}
                selectedId={account.id}
                labels={copy.accountCard.planLabels}
                onSelect={(id) => onVariant(row.id, id)}
              />
              <AccountResetCredits account={account} text={text} />
            </div>
            <div className="accountStateLine">
              {status !== "using" && status !== "available" ? (
                <span
                  className={`statusText status-${status}`}
                  title={
                    accountIssueReason(account, text.issueFallbackReason) ??
                    statusLabel(status, text)
                  }
                >
                  <span className="statusDot" />
                  {statusLabel(status, text)}
                </span>
              ) : null}
              {account.sourceKind === "relay" && account.balanceText ? (
                <span className="relayBalance">{account.balanceText}</span>
              ) : null}
              <UsageFreshnessBadge
                account={account}
                refreshing={actions.usageRefreshing}
                showInitialRefresh={actions.showInitialUsageRefresh}
                refreshError={actions.usageRefreshError}
                locale={locale}
                copy={copy.accountsGrid}
                compact
              />
            </div>
          </div>
        </div>
      </td>
      <td>
        <QuotaMeter
          label={text.fiveHourUsage}
          window={account.usage?.fiveHour ?? null}
          locale={locale}
          compact
        />
      </td>
      <td>
        <QuotaMeter
          label={text.weekUsage}
          window={account.usage?.oneWeek ?? null}
          locale={locale}
          compact
        />
      </td>
      <td className="accountProxyCell">
        <AccountProxyToggle
          account={account}
          disabled={actions.authBusy}
          onToggle={actions.onToggleApiProxy}
        />
      </td>
      <td className="accountActionCell">
        <div className="rowActions">
          {!account.isCurrent ? (
            <button
              type="button"
              className="rowSwitchButton"
              disabled={actions.authBusy}
              onClick={() => void onSwitch(account)}
            >
              {actions.switchingId === account.id
                ? copy.accountCard.launching
                : text.switchAccount}
            </button>
          ) : null}
          <AccountActionMenu
            account={account}
            actions={actions}
            text={text}
            onInspect={() => onInspect(account.id)}
          />
        </div>
      </td>
    </tr>
  );
}
