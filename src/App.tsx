import { startTransition, Suspense, useCallback, useState } from "react";
import "./App.css";
import { AppDialogs } from "./components/workspace/AppDialogs";
import { AccountsView } from "./components/workspace/AccountsView";
import { useCodexController } from "./hooks/useCodexController";
import { useThemeMode } from "./hooks/useThemeMode";
import { useAppNavigation } from "./hooks/useAppNavigation";
import type { AppTab } from "./types/workspace";
import { ClassicTopBar } from "./components/classic/ClassicTopBar";
import { AnalyticsView, ProxyView, SettingsView } from "./components/workspace/viewModules";

function AppWorkspace() {
  const [activeTab, setActiveTab] = useState<AppTab>("accounts");
  const selectTab = useCallback((tab: AppTab) => {
    // Keep the current page visible while a lazy view is loading.
    startTransition(() => setActiveTab(tab));
  }, []);
  const openAccountSearch = useCallback(() => {
    document.querySelector<HTMLInputElement>("[data-account-search]")?.focus();
  }, []);
  const { themeMode, toggleTheme } = useThemeMode();
  const c = useCodexController(activeTab);
  useAppNavigation(activeTab, selectTab, c, openAccountSearch);
  const isMacos = /Macintosh|Mac OS X/i.test(navigator.userAgent);
  const quotaOnboardingPlatform = /Windows/i.test(navigator.userAgent)
    ? "windows"
    : isMacos
      ? "macos"
      : null;
  const refresh = () => {
    if (activeTab === "analytics") {
      void c.refreshCostAnalytics(false);
      void c.refreshTokenUsage(true);
    } else if (activeTab === "proxy") void c.loadApiProxyStatus();
    else {
      void c.refreshUsage(false);
      void c.refreshTokenUsage(false);
    }
  };

  return (
    <div
      className={`shell classicApp${isMacos ? " isMacos" : ""}${c.mainWindowVisible ? "" : " isUiInactive"}`}
    >
      <main className="panel">
        <div className="workspaceMain">
          <ClassicTopBar
            activeTab={activeTab}
            onSelectTab={selectTab}
            themeMode={themeMode}
            onToggleTheme={toggleTheme}
            onRefresh={refresh}
            refreshing={
              activeTab === "analytics"
                ? c.costAnalyticsLoading
                : c.refreshing || c.refreshingTokenUsage
            }
            onGoHome={() => selectTab("accounts")}
            showRefresh={activeTab !== "settings"}
          />
          <section
            className="viewStage"
            id="workspace-content"
            aria-label={activeTab}
          >
            <Suspense
              fallback={
                <div className="workspaceLoading" role="status">
                  …
                </div>
              }
            >
              {activeTab === "accounts" ? (
                <AccountsView c={c} />
              ) : activeTab === "analytics" ? (
                <AnalyticsView c={c} />
              ) : activeTab === "proxy" ? (
                <ProxyView c={c} />
              ) : (
                <SettingsView
                  c={c}
                  themeMode={themeMode}
                  toggleTheme={toggleTheme}
                />
              )}
            </Suspense>
          </section>
        </div>
        <AppDialogs
          c={c}
          themeMode={themeMode}
          quotaOnboardingPlatform={quotaOnboardingPlatform}
        />
      </main>
    </div>
  );
}

export default AppWorkspace;
