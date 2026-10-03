import { useI18n } from "../../i18n/I18nProvider";
import { getCompactTableCopy } from "../../i18n/compactTableCopy";
import { formatQuotaTime } from "../../utils/quotaTime";
import type { AccountsGridProps } from "./types";
import { AccountsMoreMenu } from "./AccountsMoreMenu";

export function AccountsFooter({ actions }: { actions: AccountsGridProps }) {
  const { locale } = useI18n();
  const text = getCompactTableCopy(locale);
  const latestFetch = actions.accounts.reduce(
    (latest, account) => Math.max(latest, account.usage?.fetchedAt ?? 0),
    0,
  );
  const fetched = latestFetch > 0 ? formatQuotaTime(latestFetch) : null;
  const zone = Intl.DateTimeFormat().resolvedOptions().timeZone;
  return (
    <footer className="accountListFoot">
      <span className="accountCount">
        {text.accounts(actions.accounts.length)}
      </span>
      <span className="accountLastUpdated" title={text.updateHelp}>
        {fetched ? (
          <>
            {text.latestUpdate}{" "}
            <time
              dateTime={fetched.iso}
              title={`${fetched.date} ${fetched.time}`}
            >
              {fetched.time}
            </time>
          </>
        ) : (
          text.notUpdated
        )}
      </span>
      <span className="accountTimeZone" title={text.localTime}>
        {text.resetTime} · {zone}
      </span>
      <AccountsMoreMenu actions={actions} />
    </footer>
  );
}
