import { type FormEvent, useMemo, useState } from "react";
import { useI18n } from "../i18n/I18nProvider";
import type { AccountSummary } from "../types/app";
import {
  formatPlan,
  formatWindowLabel,
  percent,
  planTone,
} from "../utils/usage";

type AccountCardProps = {
  accounts: AccountSummary[];
  switchingId: string | null;
  renamingAccountId: string | null;
  pendingDeleteId: string | null;
  onRename: (account: AccountSummary, label: string) => Promise<boolean>;
  onSwitch: (account: AccountSummary) => void;
  onDelete: (account: AccountSummary) => void;
};

type UsageDialProps = {
  accent: "hot" | "cool";
  centerLabel: string;
  label: string;
  resetTitle: string;
  resetValue: string;
  usedPercent: number | null | undefined;
};

function LaunchIcon({ spinning }: { spinning: boolean }) {
  if (spinning) {
    return (
      <svg
        className="iconGlyph isSpinning"
        viewBox="0 0 24 24"
        aria-hidden="true"
        focusable="false"
      >
        <path d="M21 12a9 9 0 1 1-2.64-6.36" />
        <path d="M21 3v6h-6" />
      </svg>
    );
  }

  return (
    <svg className="iconGlyph" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M7 5v14l11-7z" />
    </svg>
  );
}

function EditIcon() {
  return (
    <svg className="iconGlyph" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M12 20h9" />
      <path d="M16.5 3.5a2.12 2.12 0 1 1 3 3L7 19l-4 1 1-4z" />
    </svg>
  );
}

function UsageDial({
  accent,
  centerLabel,
  label,
  resetTitle,
  resetValue,
  usedPercent,
}: UsageDialProps) {
  const radius = 29;
  const circumference = 2 * Math.PI * radius;
  const normalized =
    usedPercent === undefined || usedPercent === null || Number.isNaN(usedPercent)
      ? 0
      : Math.max(0, Math.min(100, usedPercent));
  const dashOffset = circumference * (1 - normalized / 100);

  return (
    <section className={`usageDial usageDial-${accent}`}>
      <strong className="usageDialLabel">{label}</strong>
      <div className="usageDialChart" aria-hidden="true">
        <svg className="usageDialSvg" viewBox="0 0 84 84">
          <circle className="usageDialTrack" cx="42" cy="42" r={radius} />
          <circle
            className="usageDialProgress"
            cx="42"
            cy="42"
            r={radius}
            style={{
              strokeDasharray: circumference,
              strokeDashoffset: dashOffset,
            }}
          />
        </svg>
        <div className="usageDialCenter">
          <strong>{percent(usedPercent)}</strong>
          <span>{centerLabel}</span>
        </div>
      </div>
      <div className="usageDialReset">
        <span>{resetTitle}</span>
        <strong>{resetValue}</strong>
      </div>
    </section>
  );
}

function formatResetValue(epochSec: number | null | undefined, locale?: string) {
  if (!epochSec) {
    return "--";
  }

  const value = new Date(epochSec * 1000);
  return value.toLocaleString(locale, {
    year: "numeric",
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function pickDefaultAccount(accounts: AccountSummary[]): AccountSummary | null {
  const current = accounts.find((account) => account.isCurrent);
  if (current) {
    return current;
  }
  return accounts[0] ?? null;
}

export function AccountCard({
  accounts,
  switchingId,
  renamingAccountId,
  pendingDeleteId,
  onRename,
  onSwitch,
  onDelete,
}: AccountCardProps) {
  const { copy, locale } = useI18n();
  const [preferredSelectedId, setPreferredSelectedId] = useState<string | null>(
    () => pickDefaultAccount(accounts)?.id ?? null,
  );
  const [isEditingAlias, setIsEditingAlias] = useState(false);
  const [draftLabel, setDraftLabel] = useState("");

  const selectedAccount = useMemo(
    () =>
      (switchingId && accounts.find((account) => account.id === switchingId)) ||
      (pendingDeleteId && accounts.find((account) => account.id === pendingDeleteId)) ||
      accounts.find((account) => account.isCurrent) ||
      (preferredSelectedId && accounts.find((account) => account.id === preferredSelectedId)) ||
      pickDefaultAccount(accounts),
    [accounts, pendingDeleteId, preferredSelectedId, switchingId],
  );

  if (!selectedAccount) {
    return null;
  }

  const usage = selectedAccount.usage;
  const fiveHour = usage?.fiveHour ?? null;
  const oneWeek = usage?.oneWeek ?? null;
  const normalizedPlan = selectedAccount.planType || usage?.planType;
  const tone = planTone(normalizedPlan);
  const isSwitching = switchingId === selectedAccount.id;
  const isRenaming = renamingAccountId === selectedAccount.accountKey;
  const isDeletePending = pendingDeleteId === selectedAccount.id;
  const launchLabel = isSwitching ? copy.accountCard.launching : copy.accountCard.launch;
  const fiveHourReset = formatResetValue(fiveHour?.resetAt, locale);
  const oneWeekReset = formatResetValue(oneWeek?.resetAt, locale);
  const normalizedDraftLabel = draftLabel.trim();
  const canSaveAlias =
    normalizedDraftLabel.length > 0 &&
    normalizedDraftLabel !== selectedAccount.label.trim() &&
    !isRenaming;

  const handleLaunch = () => {
    if (isSwitching) return;
    onSwitch(selectedAccount);
  };

  const handleSelectAccount = (account: AccountSummary) => {
    setPreferredSelectedId(account.id);
  };

  const handleStartAliasEdit = () => {
    setDraftLabel(selectedAccount.label);
    setIsEditingAlias(true);
  };

  const handleCancelAliasEdit = () => {
    if (isRenaming) {
      return;
    }
    setDraftLabel(selectedAccount.label);
    setIsEditingAlias(false);
  };

  const handleAliasSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!canSaveAlias) {
      return;
    }

    const updated = await onRename(selectedAccount, normalizedDraftLabel);
    if (updated) {
      setIsEditingAlias(false);
    }
  };

  return (
    <article
      className={`accountCard tone-${tone} ${selectedAccount.isCurrent ? "isCurrent" : ""} ${
        isSwitching ? "isSwitching" : ""
      }`}
    >
      <header className="cardHeader">
        <div className="cardIdentity">
          <div className="cardBadges">
            {accounts.map((account) => {
              const variantPlan = formatPlan(
                account.planType || account.usage?.planType,
                copy.accountCard.planLabels,
              );
              const isSelected = account.id === selectedAccount.id;
              return (
                <button
                  key={account.id}
                  type="button"
                  className={`cardBadge planBadge planBadgeButton ${
                    isSelected ? "isSelected" : ""
                  } ${account.isCurrent ? "isCurrent" : ""}`}
                  onClick={() => handleSelectAccount(account)}
                  aria-pressed={isSelected}
                  title={
                    account.isCurrent
                      ? `${variantPlan} · ${copy.accountCard.currentStamp}`
                      : variantPlan
                  }
                >
                  {variantPlan}
                  {account.isCurrent && (
                    <span className="planCurrentGlass" aria-hidden="true">
                      {copy.accountCard.currentStamp}
                    </span>
                  )}
                </button>
              );
            })}
          </div>
          {isEditingAlias ? (
            <form className="cardAliasEditor" onSubmit={handleAliasSubmit}>
              <label className="visuallyHidden" htmlFor={`account-alias-${selectedAccount.id}`}>
                {copy.accountCard.aliasInputLabel}
              </label>
              <input
                id={`account-alias-${selectedAccount.id}`}
                value={draftLabel}
                maxLength={60}
                autoFocus
                disabled={isRenaming}
                onChange={(event) => setDraftLabel(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Escape") {
                    event.preventDefault();
                    handleCancelAliasEdit();
                  }
                }}
              />
              <div className="cardAliasActions">
                <button
                  type="submit"
                  className="primary cardAliasAction"
                  disabled={!canSaveAlias}
                >
                  {copy.accountCard.saveAlias}
                </button>
                <button
                  type="button"
                  className="ghost cardAliasAction"
                  onClick={handleCancelAliasEdit}
                  disabled={isRenaming}
                >
                  {copy.accountCard.cancelAlias}
                </button>
              </div>
            </form>
          ) : (
            <h3 className={selectedAccount.isCurrent ? "nameCurrent" : ""}>
              {selectedAccount.label}
            </h3>
          )}
        </div>
        <div className="cardActions">
          <button
            type="button"
            className="cardEditIcon"
            onClick={handleStartAliasEdit}
            disabled={isEditingAlias || isRenaming}
            aria-label={copy.accountCard.editAlias}
            title={copy.accountCard.editAlias}
          >
            <EditIcon />
          </button>
          <button
            type="button"
            className={`cardDeleteIcon ${isDeletePending ? "isPending" : ""}`}
            onClick={() => onDelete(selectedAccount)}
            aria-label={isDeletePending ? copy.accountCard.deleteConfirm : copy.accountCard.delete}
            title={isDeletePending ? copy.accountCard.deleteConfirm : copy.accountCard.delete}
          >
            <svg className="iconGlyph" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
              <path d="M3 6h18" />
              <path d="M8 6V4h8v2" />
              <path d="M19 6l-1 14H6L5 6" />
              <path d="M10 11v6" />
              <path d="M14 11v6" />
            </svg>
          </button>
        </div>
      </header>

      <div className="usageGrid">
        <UsageDial
          accent="hot"
          centerLabel={copy.accountCard.used}
          label={formatWindowLabel(fiveHour, {
            fallback: copy.accountCard.fiveHourFallback,
            oneWeek: copy.accountCard.oneWeekLabel,
            hourSuffix: copy.accountCard.hourSuffix,
            minuteSuffix: copy.accountCard.minuteSuffix,
          })}
          resetTitle={copy.accountCard.resetAt}
          resetValue={fiveHourReset}
          usedPercent={fiveHour?.usedPercent}
        />
        <UsageDial
          accent="cool"
          centerLabel={copy.accountCard.used}
          label={formatWindowLabel(oneWeek, {
            fallback: copy.accountCard.oneWeekFallback,
            oneWeek: copy.accountCard.oneWeekLabel,
            hourSuffix: copy.accountCard.hourSuffix,
            minuteSuffix: copy.accountCard.minuteSuffix,
          })}
          resetTitle={copy.accountCard.resetAt}
          resetValue={oneWeekReset}
          usedPercent={oneWeek?.usedPercent}
        />
      </div>

      <footer className="cardFooter">
        {selectedAccount.usageError && <p className="errorText">{selectedAccount.usageError}</p>}
        <button
          className={`ghost cardLaunchButton ${isSwitching ? "isBusy" : ""}`}
          onClick={handleLaunch}
          disabled={isSwitching}
          aria-label={launchLabel}
          title={isSwitching ? `${copy.accountCard.launching}...` : copy.accountCard.launch}
        >
          <LaunchIcon spinning={isSwitching} />
          <span>{launchLabel}</span>
        </button>
      </footer>
    </article>
  );
}
