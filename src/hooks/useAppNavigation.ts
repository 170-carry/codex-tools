import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppTab, CodexController } from "../types/workspace";

export function useAppNavigation(
  activeTab: AppTab,
  onSelectTab: (tab: AppTab) => void,
  c: CodexController,
  onFindAccount: () => void,
) {
  const {
    checkForAppUpdate,
    updateSettings,
    onOpenAddDialog,
    refreshUsage,
    refreshTokenUsage,
    refreshCostAnalytics,
    loadApiProxyStatus,
    mainWindowVisible,
    tokenUsage,
  } = c;
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (
        document.querySelector(
          "dialog[open], [role='dialog'][aria-modal='true'], [role='alertdialog'][aria-modal='true']",
        )
      )
        return;
      const modifier = /Mac/i.test(navigator.platform)
        ? event.metaKey
        : event.ctrlKey;
      if (!modifier || event.altKey || event.shiftKey) return;
      const key = event.key.toLowerCase();
      const tabs: AppTab[] = ["accounts", "analytics", "proxy", "settings"];
      if (/^[1-4]$/.test(key)) {
        event.preventDefault();
        onSelectTab(tabs[Number(key) - 1]);
      } else if (key === ",") {
        event.preventDefault();
        onSelectTab("settings");
      } else if (key === "n") {
        event.preventDefault();
        onOpenAddDialog();
      } else if (key === "f" && activeTab === "accounts") {
        event.preventDefault();
        onFindAccount();
      } else if (key === "r") {
        event.preventDefault();
        if (activeTab === "analytics") {
          void refreshCostAnalytics(false);
          void refreshTokenUsage(true);
        } else if (activeTab === "proxy") void loadApiProxyStatus();
        else {
          void refreshUsage(false);
          void refreshTokenUsage(false);
        }
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [
    activeTab,
    onSelectTab,
    onOpenAddDialog,
    onFindAccount,
    refreshUsage,
    refreshTokenUsage,
    refreshCostAnalytics,
    loadApiProxyStatus,
  ]);
  useEffect(() => {
    let disposed = false;
    const unlisteners: UnlistenFn[] = [];
    const register = async () => {
      const registrations = await Promise.allSettled([
        listen("app-menu-open-settings", () => onSelectTab("settings")),
        listen("app-menu-check-update", () => void checkForAppUpdate(false)),
        listen(
          "app-menu-open-quota-onboarding",
          () =>
            void updateSettings(
              { macosQuotaOnboardingCompleted: false },
              { silent: true, throwOnError: true, keepInteractive: true },
            ),
        ),
      ]);
      for (const result of registrations)
        if (result.status === "fulfilled") {
          if (disposed) result.value();
          else unlisteners.push(result.value);
        }
    };
    void register();
    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [checkForAppUpdate, onSelectTab, updateSettings]);
  useEffect(() => {
    if (
      (activeTab !== "accounts" && activeTab !== "analytics") ||
      !mainWindowVisible
    )
      return;
    const updatedAtMs = (tokenUsage?.updatedAt ?? 0) * 1000;
    if (updatedAtMs > 0 && Date.now() < updatedAtMs + 5 * 60 * 1000) return;
    void refreshTokenUsage(true);
  }, [activeTab, mainWindowVisible, refreshTokenUsage, tokenUsage]);
}
