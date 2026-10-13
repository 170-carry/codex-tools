import { useCallback, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n/I18nProvider";
import type { AccountSummary, Notice } from "../types/app";

type ResetCreditResult = {
  code: string;
  windowsReset: number;
  accounts: AccountSummary[];
  refreshError: string | null;
};

export function useResetCredits({
  busy,
  applyAccounts,
  setNotice,
  localizeError,
}: {
  busy: boolean;
  applyAccounts: (accounts: AccountSummary[]) => unknown;
  setNotice: (notice: Notice) => void;
  localizeError: (error: string) => string;
}) {
  const { copy } = useI18n();
  const [resettingAccountId, setResettingAccountId] = useState<string | null>(null);
  const inFlight = useRef(false);

  const onUseResetCredit = useCallback(
    async (account: AccountSummary, creditId: string) => {
      if (busy || inFlight.current || account.sourceKind === "relay") return false;
      inFlight.current = true;
      setResettingAccountId(account.id);
      try {
        const result = await invoke<ResetCreditResult>("use_reset_credit", {
          id: account.id,
          creditId,
        });
        applyAccounts(result.accounts);
        const messages: Record<string, string> = {
          reset: copy.resetCredit.reset,
          nothing_to_reset: copy.resetCredit.nothingToReset,
          no_credit: copy.resetCredit.noCredit,
          already_redeemed: copy.resetCredit.alreadyRedeemed,
        };
        setNotice({
          type: result.code === "reset" && !result.refreshError ? "ok" : "info",
          message: (messages[result.code] ?? copy.resetCredit.unknown) +
            (result.refreshError ? ` · ${copy.resetCredit.refreshFailed}` : ""),
        });
        return true;
      } catch (error) {
        setNotice({ type: "error", message: localizeError(String(error)) });
        return false;
      } finally {
        inFlight.current = false;
        setResettingAccountId(null);
      }
    },
    [applyAccounts, busy, copy.resetCredit, localizeError, setNotice],
  );

  return { resettingAccountId, onUseResetCredit };
}
