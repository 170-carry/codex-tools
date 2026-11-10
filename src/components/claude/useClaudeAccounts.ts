import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ClaudeAccounts } from "./types";

export function useClaudeAccounts(refreshKey: number) {
  const [data, setData] = useState<ClaudeAccounts>({ accounts: [] });
  const [error, setError] = useState("");
  const [refreshing, setRefreshing] = useState(false);
  const inFlight = useRef(false);
  const mounted = useRef(false);
  const load = useCallback(async (force = false) => {
    if (inFlight.current) return;
    inFlight.current = true;
    setRefreshing(true);
    try {
      const cached = await invoke<ClaudeAccounts>("claude_list_accounts");
      if (mounted.current) setData(cached);
      const next = await invoke<ClaudeAccounts>("claude_refresh_accounts", {
        force,
      });
      if (mounted.current) {
        setData(next);
        setError("");
      }
    } catch (e) {
      if (mounted.current) setError(String(e));
    } finally {
      inFlight.current = false;
      if (mounted.current) setRefreshing(false);
    }
  }, []);
  useEffect(() => {
    mounted.current = true;
    void load(refreshKey > 0);
    const refresh = () => {
      if (!document.hidden) void load();
    };
    const timer = window.setInterval(refresh, 300000);
    document.addEventListener("visibilitychange", refresh);
    window.addEventListener("focus", refresh);
    return () => {
      mounted.current = false;
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", refresh);
      window.removeEventListener("focus", refresh);
    };
  }, [load, refreshKey]);
  return { data, setData, error, setError, refreshing, load };
}
