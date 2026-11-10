import { FeedbackHost } from "./components/ui/FeedbackHost";
import type { AccountProvider } from "./components/workspace/ProviderSwitch";
import "./styles/claude.css";
import { lazy, startTransition, Suspense, useCallback, useState } from "react";
import "./App.css";
import "./styles/authorization-controls.css";
import "./styles/claude-page.css";
import "./styles/hig/index.css";
import { AppDialogs } from "./components/workspace/AppDialogs";
import { AccountsView } from "./components/workspace/AccountsView";
import { useCodexController } from "./hooks/useCodexController";
import { useThemeMode } from "./hooks/useThemeMode";
import { useAppNavigation } from "./hooks/useAppNavigation";
import type { AppTab } from "./types/workspace";
import { ClassicTopBar } from "./components/classic/ClassicTopBar";
import { LocalProjectRecoveryView } from "./components/LocalProjectDialogs";
import { AnalyticsView, ProxyView, SettingsView } from "./components/workspace/viewModules";

const ClaudeAccountsView = lazy(() => import("./components/claude/ClaudeAccountsView").then((m) => ({ default: m.ClaudeAccountsView })));

function AppWorkspace() {
  const [provider, setProvider] = useState<AccountProvider>(() => localStorage.getItem("codex-tools.account-provider") === "claude" ? "claude" : "codex");
  const [claudeRefreshKey, setClaudeRefreshKey] = useState(0);
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
  useAppNavigation(activeTab, selectTab, c, openAccountSearch, provider === "codex");
  const isMacos = /Macintosh|Mac OS X/i.test(navigator.userAgent);
  const quotaOnboardingPlatform = /Windows/i.test(navigator.userAgent)
    ? "windows"
    : isMacos
      ? "macos"
      : null;
  if (c.recoveryRequired) {
    return <div className="shell classicApp higApp"><main className="panel">
        <FeedbackHost />
      <LocalProjectRecoveryView
        recovering={c.recoveringLocalProjects}
        onRecover={() => void c.onRecoverLocalProjects()}
        notice={c.notice}
      />
      <AppDialogs c={c} themeMode={themeMode} quotaOnboardingPlatform={quotaOnboardingPlatform} />
    </main></div>;
  }
  const changeProvider = (next: AccountProvider) => {
    if (c.oauthWaitingForCallback) void c.onCancelOauthLogin();
    c.onCloseAddDialog();
    setProvider(next);
    localStorage.setItem("codex-tools.account-provider", next);
    selectTab("accounts");
  };
  const refresh = () => {
    if (provider === "claude") { setClaudeRefreshKey((key) => key + 1); return; }
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
      className={`shell classicApp higApp${provider === "claude" ? " isClaude" : ""}${isMacos ? " isMacos" : ""}${c.mainWindowVisible ? "" : " isUiInactive"}`}
    >
      <main className="panel">
        <FeedbackHost />
        <div className="workspaceMain">
          <ClassicTopBar
            provider={provider}
            onProviderChange={changeProvider}
            activeTab={activeTab}
            onSelectTab={selectTab}
            themeMode={themeMode}
            onToggleTheme={toggleTheme}
            onRefresh={refresh}
            refreshing={
              provider === "claude" ? false : activeTab === "analytics"
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
                provider === "claude" ? <ClaudeAccountsView refreshKey={claudeRefreshKey} /> : <AccountsView c={c} />
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
