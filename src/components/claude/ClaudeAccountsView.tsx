import { Presence } from "../ui/Presence";
import { SearchField } from "../ui/SearchField";
import { AccountsWorkspace } from "../ui/AccountsWorkspace";
import { claudeText } from "../../i18n/claudeCopy";
import { PageFrame } from "../ui/PageFrame";
import { UtilityDialog } from "../workspace/UtilityDialog";
import { ClaudeAccountDetails } from "./ClaudeAccountDetails";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../../i18n/I18nProvider";
import { ClaudePageHeader, ClaudeEmptyState } from "./ClaudePageHeader";
import { ClaudeAccountCard } from "./ClaudeAccountCard";
import { useClaudeAccounts } from "./useClaudeAccounts";
import { ClaudeImportDialog } from "./ClaudeImportDialog";
import type { ClaudeAccount, ClaudeAccounts } from "./types";

export function ClaudeAccountsView({ refreshKey }: { refreshKey: number }) {
  const { locale } = useI18n();
  const ct = claudeText(locale);
  const { data, setData, error, setError, load, refreshing } =
    useClaudeAccounts(refreshKey);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const selected = data.accounts.find(a => a.id === selectedId);
  const [query, setQuery] = useState("");
  const [dialog, setDialog] = useState<{
    account: ClaudeAccount | null;
  } | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("");
  const [deleteAccount, setDeleteAccount] = useState<ClaudeAccount | null>(
    null,
  );
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (
        !(event.metaKey || event.ctrlKey) ||
        event.altKey ||
        event.shiftKey ||
        dialog ||
        deleteAccount
      )
        return;
      if (event.key.toLowerCase() === "n") {
        event.preventDefault();
        setDialog({ account: null });
      }
      if (event.key.toLowerCase() === "r") {
        event.preventDefault();
        void load(true);
      }
      if (event.key.toLowerCase() === "f") {
        event.preventDefault();
        document
          .querySelector<HTMLInputElement>("[data-claude-search]")
          ?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dialog, deleteAccount, load]);
  const run = async (command: string, accountId: string) => {
    setBusy(true);
    setError("");
    setNotice("");
    try {
      setData(await invoke<ClaudeAccounts>(command, { accountId }));
      if (command === "claude_switch_account")
        setNotice(
          ct("Claude Code login updated. Open a new Claude Code session to use it."),
        );
      if (command === "claude_switch_desktop_account")
        setNotice(
          ct("Claude desktop restarted with the selected login; local project records were preserved."),
        );
      if (command === "claude_delete_account") setDeleteAccount(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const accounts = data.accounts.filter((a) =>
    `${a.label} ${a.email ?? ""}`.toLowerCase().includes(query.toLowerCase()),
  );
  return (
    <PageFrame kind="list" className="accountsPage claudeAccountsView">
    <AccountsWorkspace summary={<ClaudePageHeader accounts={data.accounts} />} inspector={selected ? <ClaudeAccountDetails account={selected} onClose={() => setSelectedId(null)} /> : undefined}>
      <section className="accountListPanel">
        <header className="accountToolbar">
          <SearchField className="accountSearch" data-claude-search value={query} onValueChange={setQuery}
            aria-label={ct("Search Claude accounts")} placeholder={ct("Search accounts / email")} />
          <div className="accountToolbarActions">
          <button
            className="ghost"
            disabled={busy || refreshing}
            onClick={() => void load(true)}
          >
            {refreshing
              ? ct("Refreshing…")
              : ct("Refresh all")}
          </button>
          <button
            className="primary"
            type="button"
            disabled={busy || refreshing}
            onClick={() => setDialog({ account: null })}
          >
            ＋ {ct("Add account")}
          </button>
          </div>
        </header>
        {error && (
          <p role="alert" className="claudeError">
            {error}
          </p>
        )}
        {notice && <p role="status" className="uiInlineNotice">{notice}</p>}
        <div className="accountRows">
          {accounts.map((a) => (
            <ClaudeAccountCard
              key={a.id}
              a={a}
              busy={busy || refreshing}
              run={run}
              onReauthorize={(account) => setDialog({ account })}
              onDelete={setDeleteAccount}
              onInspect={() => setSelectedId(a.id)}
              selected={selectedId === a.id}
            />
          ))}
        </div>
        {!accounts.length && (
          <ClaudeEmptyState filtered={data.accounts.length > 0} onAdd={() => setDialog({ account: null })} />
        )}
      </section>
      </AccountsWorkspace>
      <Presence present={!!dialog}>{dialog && (
        <ClaudeImportDialog
          account={dialog.account}
          onSaved={(next) => {
            setData(next);
            setDialog(null);
            void load(true);
          }}
          onClose={() => setDialog(null)}
        />
      )}
      </Presence>
      <Presence present={!!deleteAccount}>{deleteAccount && (
        <UtilityDialog role="alertdialog" title={ct("Delete saved account?")} description={deleteAccount.label} closeLabel={ct("Close")} onClose={() => setDeleteAccount(null)} dismissible={!busy} showClose={false} actions={<><button className="ghost" disabled={busy} onClick={() => setDeleteAccount(null)}>{ct("Cancel")}</button><button className="danger" disabled={busy} onClick={() => void run("claude_delete_account", deleteAccount.id)}>{ct("Delete")}</button></>}>
          <p>{ct("This removes it from the account list. Current logins and local workspaces are kept.")}</p>
        </UtilityDialog>
      )}
      </Presence>
    </PageFrame>
  );
}
