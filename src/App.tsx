import { startTransition, Suspense, useCallback, useState } from "react";
import "./App.css";
import { AppTopBar } from "./components/AppTopBar";
import { AppDialogs } from "./components/workspace/AppDialogs";
import { AccountsView } from "./components/workspace/AccountsView";
import { useCodexController } from "./hooks/useCodexController";
import { useThemeMode } from "./hooks/useThemeMode";
import { useAppNavigation } from "./hooks/useAppNavigation";
import type { AppTab } from "./types/workspace";
import { AppLayoutProvider } from "./components/layout/AppLayoutProvider";
import { useAppLayout } from "./hooks/useAppLayout";
import { ClassicTopBar } from "./components/classic/ClassicTopBar";
import { AnalyticsView, ProxyView, SettingsView, GitActivityView } from "./components/workspace/viewModules";

function AppWorkspace() {
  const { layout } = useAppLayout();
  const [activeTab, setActiveTab] = useState<AppTab>("git");
  const selectTab = useCallback((tab: AppTab) => {
    // Keep the current page visible while a lazy view is loading.
    startTransition(() => setActiveTab(tab));
  }, []);
  const [accountSearchOpen, setAccountSearchOpen] = useState(false);
  const openAccountSearch = useCallback(() => {
    setAccountSearchOpen(true);
    window.requestAnimationFrame(() =>
      document
        .querySelector<HTMLInputElement>("[data-account-search]")
        ?.focus(),
    );
  }, []);
  const closeAccountSearch = () => {
    setAccountSearchOpen(false);
    document.querySelector<HTMLButtonElement>(".accountSearchButton")?.focus();
  };
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
      className={`shell ${layout === "compact" ? "nativeApp" : "classicApp"}${isMacos ? " isMacos" : ""}${c.mainWindowVisible ? "" : " isUiInactive"}`}
    >
      <main className="panel">
        <div className="workspaceMain">
          {layout === "classic" ? (
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
              onGoHome={() => selectTab("git")}
              showRefresh={activeTab !== "settings" && activeTab !== "git"}
            />
          ) : (
            <AppTopBar
              activeTab={activeTab}
              onSelectTab={selectTab}
              onRefresh={refresh}
              refreshing={
                activeTab === "analytics"
                  ? c.costAnalyticsLoading
                  : c.refreshing || c.refreshingTokenUsage
              }
              showRefresh={activeTab !== "settings" && activeTab !== "git"}
              searchOpen={accountSearchOpen}
              onSearch={openAccountSearch}
              onAddAccount={c.onOpenAddDialog}
            />
          )}
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
                <AccountsView
                  c={c}
                  searchVisible={accountSearchOpen}
                  onCloseSearch={closeAccountSearch}
                  onShowAnalytics={() => selectTab("analytics")}
                />
              ) : activeTab === "analytics" ? (
                <AnalyticsView c={c} />
              ) : activeTab === "proxy" ? (
                <ProxyView c={c} />
              ) : activeTab === "git" ? null : (
                <SettingsView
                  c={c}
                  themeMode={themeMode}
                  toggleTheme={toggleTheme}
                />
              )}
              <div hidden={activeTab !== "git"}><GitActivityView /></div>
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

function App() {
  return (
    <AppLayoutProvider>
      <AppWorkspace />
    </AppLayoutProvider>
  );
}

export default App;
