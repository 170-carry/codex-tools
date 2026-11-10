import { Button } from "../ui/Button";
import { claudeText } from "../../i18n/claudeCopy";
import { ImportMethods } from "../ui/ImportMethods";
import { UtilityDialog } from "../workspace/UtilityDialog";
import { savedAuthorizationApp } from "../../utils/authorizationApp";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../../i18n/I18nProvider";
import { AuthorizationAppPicker } from "../accounts/import/AuthorizationAppPicker";
import { SecretInput } from "../accounts/import/SecretInput";
import type { ClaudeAccount, ClaudeAccounts } from "./types";

type Route = "desktop" | "oauth" | "current" | "json" | "files" | "api";
export function ClaudeImportDialog({
  account,
  onSaved,
  onClose,
}: {
  account: ClaudeAccount | null;
  onSaved: (data: ClaudeAccounts) => void;
  onClose: () => void;
}) {
  const { locale } = useI18n();
  const ct = claudeText(locale);
  const [route, setRoute] = useState<Route>("oauth");
  const [url, setUrl] = useState("");
  const [code, setCode] = useState("");
  const [label, setLabel] = useState(account?.label ?? "");
  const [content, setContent] = useState("");
  const [files, setFiles] = useState<File[]>([]);
  const [baseUrl, setBaseUrl] = useState("https://api.anthropic.com");
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const prepare = async () => {
    setBusy(true);
    setError("");
    try {
      setUrl(await invoke<string>("claude_prepare_oauth"));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  useEffect(() => {
    let active = true;
    void invoke<string>("claude_prepare_oauth")
      .then((value) => {
        if (active) setUrl(value);
      })
      .catch((e) => {
        if (active) setError(String(e));
      });
    return () => {
      active = false;
      void invoke("claude_cancel_oauth");
    };
  }, []);
  const run = async (action: () => Promise<ClaudeAccounts>) => {
    setBusy(true);
    setError("");
    try {
      onSaved(await action());
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const save = () =>
    run(async () => {
      if (route === "desktop")
        return invoke("claude_import_desktop_account", {
          label: label || null,
        });
      if (route === "oauth")
        return invoke("claude_complete_oauth", {
          code,
          label,
          accountId: account?.id ?? null,
        });
      if (route === "api")
        return invoke("claude_save_api_account", {
          label: label || "Claude API",
          baseUrl,
          apiKey,
          model,
        });
      return invoke("claude_import_accounts", {
        current: route === "current",
        label: label || null,
        contents:
          route === "files"
            ? await Promise.all(files.map((file) => file.text()))
            : route === "json"
              ? [content]
              : [],
      });
    });
  const routes: { id: Route; text: string }[] = account
    ? [{ id: "oauth", text: "OAuth" }]
    : [
        { id: "oauth", text: "OAuth" },
        { id: "desktop", text: ct("Import desktop login") },
        { id: "current", text: ct("Import CLI login") },
        { id: "json", text: ct("Paste credentials") },
        { id: "files", text: ct("Import files") },
        { id: "api", text: "API Key" },
      ];
  const incomplete =
    route === "oauth"
      ? !url || !code.trim()
      : route === "json"
        ? !content.trim()
        : route === "files"
          ? files.length === 0
          : route === "api"
            ? !apiKey.trim() || !baseUrl.trim()
            : false;
  return (
    <UtilityDialog className="claudeImportDialog" title={account ? (ct("Reauthorize Claude")) : (ct("Add Claude account"))} closeLabel={ct("Close")} onClose={onClose} dismissible={!busy} actions={<>
          <button type="button" className="ghost" disabled={busy} onClick={onClose}>{ct("Cancel")}</button>
          <Button busy={busy} variant="primary"
            className="primary"
            type="button"
            disabled={busy || incomplete}
            onClick={() => void save()}
          >
            {busy
              ? ct("Saving…")
              : route === "api"
                ? ct("Verify and save")
                : ct("Save account")}
          </Button>
</>}>
        <div className={`uiImportWorkspace${routes.length === 1 ? " isSingleRoute" : ""}`}>
        {routes.length > 1 && <ImportMethods value={route} methods={routes.map(r => ({id: r.id, label: r.text}))} disabled={busy} onChange={next => { setRoute(next); setError(""); }} />}
        <div className="uiImportBody">
        <label>
          {ct("Account name")}
          <input
            value={label}
            disabled={busy}
            onChange={(e) => setLabel(e.target.value)}
            placeholder="Claude"
          />
        </label>
        {route === "oauth" && (
          <>
            <div className="claudeOauthActions">
              <AuthorizationAppPicker disabled={busy} />
              <Button busy={busy} variant="primary"
                type="button"
                className="primary"
                disabled={busy || !url}
                onClick={() => {
                  setError("");
                  void invoke("open_authorization_url", {
                    url,
                    application: savedAuthorizationApp()?.path ?? null,
                  }).catch((e) => setError(String(e)));
                }}
              >
                {ct("Open authorization")}
              </Button>
              <button
                type="button"
                className="ghost"
                disabled={busy}
                onClick={() => void prepare()}
              >
                {ct("New link")}
              </button>
            </div>
            <p>
              {ct("Sign in to Claude, then paste the authorization code shown on the page.")}
            </p>
            <label>
              {ct("Authorization code")}
              <textarea
                value={code}
                disabled={busy}
                onChange={(e) => setCode(e.target.value)}
                placeholder="code#state"
                spellCheck={false}
                rows={3}
              />
            </label>
            <details>
              <summary>{ct("Authorization link")}</summary>
              <input
                value={url}
                readOnly
                aria-label={ct("Authorization link")}
              />
            </details>
          </>
        )}
        {route === "desktop" && (
          <p>
            {ct("Save the current desktop login and authorize usage queries and automatic renewal. Switching accounts restarts Claude and preserves local workspaces.")}
          </p>
        )}
        {route === "current" && (
          <p>
            {ct("Import the current Claude Code login from this computer.")}
          </p>
        )}
        {route === "json" && (
          <label>
            {ct("Claude credentials JSON")}
            <textarea
              value={content}
              disabled={busy}
              onChange={(e) => setContent(e.target.value)}
              placeholder={
                '{"claudeAiOauth":{"accessToken":"…","refreshToken":"…","expiresAt":…}}'
              }
              rows={7}
              spellCheck={false}
            />
          </label>
        )}
        {route === "files" && (
          <label>
            {ct("Claude credentials files")}
            <input
              type="file"
              multiple
              accept=".json,application/json"
              disabled={busy}
              onChange={(e) => setFiles(Array.from(e.target.files ?? []))}
            />
            <span>{files.map((f) => f.name).join(", ")}</span>
          </label>
        )}
        {route === "api" && (
          <>
            <label>
              Base URL
              <input
                value={baseUrl}
                disabled={busy}
                onChange={(e) => setBaseUrl(e.target.value)}
              />
            </label>
            <label>
              API Key
              <SecretInput
                value={apiKey}
                disabled={busy}
                onChange={(e) => setApiKey(e.target.value)}
                aria-label="Claude API Key"
              />
            </label>
            <label>
              {ct("Model (optional)")}
              <input
                value={model}
                disabled={busy}
                onChange={(e) => setModel(e.target.value)}
              />
            </label>
          </>
        )}
        {error && (
          <p role="alert" className="uiFormMessage isError">
            {error}
          </p>
        )}
        </div></div>
    </UtilityDialog>
  );
}
