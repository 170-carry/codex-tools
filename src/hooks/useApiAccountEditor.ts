import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AccountSummary } from "../types/app";
import type { ProjectCommandInvoker } from "../utils/localProjectSwitch";

export type ApiAccountEdit = { label: string; baseUrl: string; modelName: string; apiKey: string | null };

export function useApiAccountEditor(onSaved: () => Promise<unknown>, options?: {
  invokeCommand: ProjectCommandInvoker;
  localizeError: (error: string) => string;
}) {
  const [account, setAccount] = useState<AccountSummary | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  return {
    account, saving, error,
    open: (next: AccountSummary) => { setError(null); setAccount(next); },
    close: () => { if (!saving) setAccount(null); },
    save: async (input: ApiAccountEdit) => {
      if (!account || saving) return;
      setSaving(true); setError(null);
      try {
        await (options?.invokeCommand ?? invoke)("update_api_account", { id: account.id, input });
        await onSaved();
        setAccount(null);
      } catch (error) { setError(options?.localizeError(String(error)) ?? String(error)); }
      finally { setSaving(false); }
    },
  };
}
