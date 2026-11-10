import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../../../i18n/I18nProvider";

import {
  savedAuthorizationApp,
  preferenceKey,
  type AuthorizationApp,
} from "../../../utils/authorizationApp";
export function AuthorizationAppPicker({
  disabled = false,
}: {
  disabled?: boolean;
}) {
  const { locale } = useI18n();
  const en = locale === "en-US";
  const [selected, setSelected] = useState(savedAuthorizationApp);
  const [apps, setApps] = useState<AuthorizationApp[]>([]);
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    void invoke<AuthorizationApp[]>("list_authorization_apps")
      .then((result) => {
        if (active) setApps(result);
      })
      .catch((e) => {
        if (active) setError(String(e));
      });
    return () => {
      active = false;
    };
  }, []);
  const options =
    selected && !apps.some((a) => a.path === selected.path)
      ? [selected, ...apps]
      : apps;
  const choose = async (path: string) => {
    try {
      setError("");
      const next =
        path === "choose"
          ? await invoke<AuthorizationApp | null>("choose_authorization_app")
          : (options.find((a) => a.path === path) ?? null);
      if (path === "choose" && !next) return;
      setSelected(next);
      if (next) localStorage.setItem(preferenceKey, JSON.stringify(next));
      else localStorage.removeItem(preferenceKey);
    } catch (e) {
      setError(String(e));
    }
  };
  return (
    <label className="authorizationAppPicker">
      <span>{en ? "Open with" : "打开方式"}</span>
      <span className="authorizationAppControl">
      <svg className="authorizationAppIcon" width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true"><rect x="3" y="4" width="18" height="16" rx="3"/><path d="M3 9h18"/><circle cx="6.5" cy="6.5" r=".6" fill="currentColor" stroke="none"/></svg>
      <select
        aria-label={en ? "Authorization application" : "授权链接打开应用"}
        value={selected?.path ?? ""}
        onChange={(e) => void choose(e.target.value)}
        disabled={disabled}
      >
        <option value="">{en ? "Default browser" : "默认浏览器"}</option>
        {options.map((a) => (
          <option key={a.path} value={a.path}>
            {a.name}
          </option>
        ))}
        <option value="choose">
          {en ? "Choose another app…" : "选择其他应用…"}
        </option>
      </select>
      <svg className="authorizationAppChevron" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="m7 10 5 5 5-5"/></svg>
      </span>
      {error && <span role="alert">{error}</span>}
    </label>
  );
}
