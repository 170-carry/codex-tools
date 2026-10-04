import { lazy, Suspense, useCallback, useState } from "react";
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

const AnalyticsView = lazy(() =>
  import("./components/workspace/AnalyticsView").then((module) => ({
    default: module.AnalyticsView,
  })),
);
const ProxyView = lazy(() =>
  import("./components/workspace/ProxyView").then((module) => ({
    default: module.ProxyView,
  })),
);
const SettingsView = lazy(() =>
  import("./components/workspace/SettingsView").then((module) => ({
    default: module.SettingsView,
  })),
);

function AppWorkspace() {
  const { layout } = useAppLayout();
  const [activeTab, setActiveTab] = useState<AppTab>("accounts");
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
  useAppNavigation(activeTab, setActiveTab, c, openAccountSearch);
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
              onSelectTab={setActiveTab}
              themeMode={themeMode}
              onToggleTheme={toggleTheme}
              onRefresh={refresh}
              refreshing={
                activeTab === "analytics"
                  ? c.costAnalyticsLoading
                  : c.refreshing || c.refreshingTokenUsage
              }
              onGoHome={() => setActiveTab("accounts")}
              showRefresh={activeTab !== "settings"}
            />
          ) : (
            <AppTopBar
              activeTab={activeTab}
              onSelectTab={setActiveTab}
              onRefresh={refresh}
              refreshing={
                activeTab === "analytics"
                  ? c.costAnalyticsLoading
                  : c.refreshing || c.refreshingTokenUsage
              }
              showRefresh={activeTab !== "settings"}
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
                  onShowAnalytics={() => setActiveTab("analytics")}
                />
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

function App() {
  return (
    <AppLayoutProvider>
      <AppWorkspace />
    </AppLayoutProvider>
  );
}

export default App;
