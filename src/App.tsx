import { useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import "./App.css";
import { AnalyticsPanel } from "./components/AnalyticsPanel";
import { ApiProxyPanel } from "./components/ApiProxyPanel";
import { AddAccountSection } from "./components/AddAccountSection";
import { AddAccountDialog } from "./components/AddAccountDialog";
import { AccountsGrid } from "./components/AccountsGrid";
import { AppTopBar } from "./components/AppTopBar";
import { DebugFloatingTool } from "./components/DebugFloatingTool";
import { MetaStrip } from "./components/MetaStrip";
import { NoticeBanner } from "./components/NoticeBanner";
import { RemoteDeployProgressToast } from "./components/RemoteDeployProgressToast";
import { SettingsPanel } from "./components/SettingsPanel";
import { UpdateBanner } from "./components/UpdateBanner";
import { useCodexController } from "./hooks/useCodexController";
import { useThemeMode } from "./hooks/useThemeMode";

type AppTab = "accounts" | "analytics" | "proxy" | "settings";
const APP_MENU_OPEN_SETTINGS_EVENT = "app-menu-open-settings";
const APP_MENU_CHECK_UPDATE_EVENT = "app-menu-check-update";

function App() {
  const [activeTab, setActiveTab] = useState<AppTab>("accounts");
  const { themeMode, toggleTheme } = useThemeMode();
  const {
    accounts,
    tokenUsage,
    tokenUsageError,
    costAnalytics,
    costAnalyticsError,
    loading,
    refreshing,
    refreshingTokenUsage,
    addDialogOpen,
    reauthorizeAccount,
    importingAccounts,
    oauthWaitingForCallback,
    exportingAccounts,
    switchingId,
    renamingAccountId,
    pendingDeleteId,
    checkingUpdate,
    installingUpdate,
    updateProgress,
    pendingUpdate,
    updateDialogOpen,
    skipPendingUpdateVersion,
    notice,
    openExternalUrl,
    settings,
    installedEditorApps,
    hasOpencodeDesktopApp,
    savingSettings,
    apiProxySupportedModels,
    apiProxyStatus,
    apiProxyKeys,
    apiProxyKeyLogs,
    apiProxyKeysLoading,
    apiProxyUsageStats,
    apiProxyUsageRange,
    apiProxyUsageMetric,
    apiProxyUsageLoading,
    apiProxyUsageClearing,
    costAnalyticsLoading,
    costAnalyticsExporting,
    costAnalyticsProgress,
    cloudflaredStatus,
    remoteProxyStatuses,
    remoteProxyLogs,
    remoteDeployProgress,
    startingApiProxy,
    stoppingApiProxy,
    refreshingApiProxyKey,
    bindingCodexProxy,
    restoringCodexProxy,
    savingApiProxyKey,
    refreshingRemoteProxyId,
    deployingRemoteProxyId,
    startingRemoteProxyId,
    stoppingRemoteProxyId,
    readingRemoteLogsId,
    installingDependencyName,
    installingDependencyTargetId,
    installingCloudflared,
    startingCloudflared,
    stoppingCloudflared,
    refreshUsage,
    refreshTokenUsage,
    loadCostAnalytics,
    refreshCostAnalytics,
    exportCostAnalytics,
    onDeleteCodexSession,
    checkForAppUpdate,
    installPendingUpdate,
    openDebugUpdateDialog,
    openManualDownloadPage,
    closeUpdateDialog,
    updateSettings,
    onOpenAddDialog,
    onReauthorizeAccount,
    onPrepareOauthLogin,
    onOpenOauthAuthorizationPage,
    onCloseAddDialog,
    onCancelOauthLogin,
    onCompleteOauthCallbackLogin,
    onImportCurrentAuth,
    onCreateApiAccount,
    onTestApiAccountConnection,
    onImportAuthFiles,
    onExportAccounts,
    loadApiProxyStatus,
    onSelectApiProxyUsageRange,
    onSelectApiProxyUsageMetric,
    onClearApiProxyUsageStats,
    onStartApiProxy,
    onStopApiProxy,
    onRefreshApiProxyKey,
    onBindCodexToApiProxy,
    onRestoreCodexProxyBinding,
    onCreateApiProxyKey,
    onUpdateApiProxyKey,
    onDeleteApiProxyKey,
    onRegenerateApiProxyKey,
    onRefreshRemoteProxyStatus,
    onDeployRemoteProxy,
    onStartRemoteProxy,
    onStopRemoteProxy,
    onReadRemoteProxyLogs,
    onPickLocalIdentityFile,
    loadCloudflaredStatus,
    onInstallCloudflared,
    onStartCloudflared,
    onStopCloudflared,
    onRenameAccountLabel,
    onToggleAccountApiProxy,
    onDelete,
    onSwitch,
    onSmartSwitch,
    onUpdateRemoteServers,
    smartSwitching,
  } = useCodexController();

  useEffect(() => {
    const isMac =
      typeof navigator !== "undefined" &&
      /Mac|iPhone|iPad|iPod/i.test(navigator.platform);
    const onKeyDown = (event: KeyboardEvent) => {
      const key = event.key.toLowerCase();
      if (key !== "r") {
        return;
      }
      const isTrigger = isMac ? event.metaKey : event.ctrlKey;
      if (!isTrigger) {
        return;
      }
      event.preventDefault();
      void refreshUsage(false);
      void refreshTokenUsage(false);
    };

    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [refreshTokenUsage, refreshUsage]);

  useEffect(() => {
    let disposed = false;
    const unlistenFns: UnlistenFn[] = [];

    const registerAppMenuListeners = async () => {
      try {
        const openSettingsUnlisten = await listen<void>(
          APP_MENU_OPEN_SETTINGS_EVENT,
          () => {
            setActiveTab("settings");
          },
        );
        const checkUpdateUnlisten = await listen<void>(
          APP_MENU_CHECK_UPDATE_EVENT,
          () => {
            void checkForAppUpdate(false);
          },
        );

        if (disposed) {
          void openSettingsUnlisten();
          void checkUpdateUnlisten();
          return;
        }

        unlistenFns.push(openSettingsUnlisten, checkUpdateUnlisten);
      } catch {
        // The app can still run in a browser-only preview where Tauri events are unavailable.
      }
    };

    void registerAppMenuListeners();

    return () => {
      disposed = true;
      for (const unlisten of unlistenFns) {
        void unlisten();
      }
    };
  }, [checkForAppUpdate]);

  const refreshAccountsView = () => {
    if (activeTab === "analytics") {
      void refreshCostAnalytics(false);
      return;
    }
    void refreshUsage(false);
    void refreshTokenUsage(false);
    void loadCostAnalytics(true);
  };

  return (
    <div className="shell">
      <div className="ambient" />
      <main className="panel">
        <AppTopBar
          activeTab={activeTab}
          onSelectTab={setActiveTab}
          themeMode={themeMode}
          onToggleTheme={toggleTheme}
          onRefresh={refreshAccountsView}
          refreshing={
            activeTab === "analytics"
              ? costAnalyticsLoading
              : refreshing || refreshingTokenUsage
          }
          onGoHome={() => setActiveTab("accounts")}
          showRefresh={activeTab === "accounts" || activeTab === "analytics"}
        />

        <AddAccountDialog
          open={addDialogOpen}
          reauthorizeAccount={reauthorizeAccount}
          importingAccounts={importingAccounts}
          oauthWaitingForCallback={oauthWaitingForCallback}
          onPrepareOauth={onPrepareOauthLogin}
          onOpenOauthPage={onOpenOauthAuthorizationPage}
          onCompleteOauth={onCompleteOauthCallbackLogin}
          onCancelOauth={onCancelOauthLogin}
          onImportCurrentAuth={onImportCurrentAuth}
          onCreateApiAccount={onCreateApiAccount}
          onTestApiConnection={onTestApiAccountConnection}
          onImportFiles={onImportAuthFiles}
          onClose={onCloseAddDialog}
        />

        <NoticeBanner notice={notice} />
        <RemoteDeployProgressToast progress={remoteDeployProgress} />
        <DebugFloatingTool onOpenUpdateDialog={openDebugUpdateDialog} />
        <UpdateBanner
          open={updateDialogOpen}
          pendingUpdate={pendingUpdate}
          updateProgress={updateProgress}
          installingUpdate={installingUpdate}
          onClose={closeUpdateDialog}
          onManualDownload={() => void openManualDownloadPage()}
          onSkipVersion={() => void skipPendingUpdateVersion()}
          onInstallNow={() => void installPendingUpdate()}
        />

        <section className="viewStage">
          {activeTab === "accounts" ? (
            <div className="accountsPage">
              <AccountsGrid
                leadingContent={
                  <MetaStrip
                    accounts={accounts}
                    exportingAccounts={exportingAccounts}
                    onExportAccounts={() => void onExportAccounts()}
                  />
                }
                toolbarActions={
                  <AddAccountSection
                    onOpenAddDialog={onOpenAddDialog}
                    onSmartSwitch={() => void onSmartSwitch()}
                    smartSwitching={smartSwitching}
                  />
                }
                accounts={accounts}
                tokenUsage={tokenUsage}
                tokenUsageError={tokenUsageError}
                loading={loading}
                exportingAccounts={exportingAccounts}
                switchingId={switchingId}
                renamingAccountId={renamingAccountId}
                pendingDeleteId={pendingDeleteId}
                onExportAll={() => void onExportAccounts()}
                onExport={(account) => void onExportAccounts(account)}
                onReauthorize={(account) => void onReauthorizeAccount(account)}
                onRename={(account, label) =>
                  onRenameAccountLabel(account, label)
                }
                onToggleApiProxy={(account, enabled) =>
                  onToggleAccountApiProxy(account, enabled)
                }
                onSwitch={(account) => void onSwitch(account)}
                onDelete={(account) => void onDelete(account)}
              />
            </div>
          ) : activeTab === "analytics" ? (
            <AnalyticsPanel
              analytics={costAnalytics}
              error={costAnalyticsError}
              loading={costAnalyticsLoading}
              exporting={costAnalyticsExporting}
              progress={costAnalyticsProgress}
              weeklyBudgetUsd={settings.codexAnalyticsWeeklyBudgetUsd}
              savingSettings={savingSettings}
              onRefresh={() => void refreshCostAnalytics(false)}
              onExport={(format) => void exportCostAnalytics(format)}
              onDeleteSession={(session) => void onDeleteCodexSession(session)}
              onUpdateWeeklyBudget={(value) =>
                updateSettings(
                  { codexAnalyticsWeeklyBudgetUsd: value },
                  { silent: true, keepInteractive: true },
                ).then(async () => {
                  await loadCostAnalytics(true);
                })
              }
            />
          ) : activeTab === "proxy" ? (
            <ApiProxyPanel
              status={apiProxyStatus}
              apiProxyKeys={apiProxyKeys}
              apiProxyKeyLogs={apiProxyKeyLogs}
              apiProxyKeysLoading={apiProxyKeysLoading}
              apiProxyUsageStats={apiProxyUsageStats}
              apiProxyUsageRange={apiProxyUsageRange}
              apiProxyUsageMetric={apiProxyUsageMetric}
              apiProxyUsageLoading={apiProxyUsageLoading}
              apiProxyUsageClearing={apiProxyUsageClearing}
              cloudflaredStatus={cloudflaredStatus}
              accountCount={accounts.length}
              autoStartEnabled={settings.autoStartApiProxy}
              savedPort={settings.apiProxyPort}
              loadBalanceMode={settings.apiProxyLoadBalanceMode}
              sequentialFiveHourLimitPercent={
                settings.apiProxySequentialFiveHourLimitPercent
              }
              apiProxySupportedModels={apiProxySupportedModels}
              apiProxyDisabledModels={settings.apiProxyDisabledModels}
              remoteServers={settings.remoteServers}
              remoteStatuses={remoteProxyStatuses}
              remoteLogs={remoteProxyLogs}
              savingSettings={savingSettings}
              starting={startingApiProxy}
              stopping={stoppingApiProxy}
              refreshingApiKey={refreshingApiProxyKey}
              bindingCodexProxy={bindingCodexProxy}
              restoringCodexProxy={restoringCodexProxy}
              savingApiProxyKey={savingApiProxyKey}
              refreshingRemoteId={refreshingRemoteProxyId}
              deployingRemoteId={deployingRemoteProxyId}
              startingRemoteId={startingRemoteProxyId}
              stoppingRemoteId={stoppingRemoteProxyId}
              readingRemoteLogsId={readingRemoteLogsId}
              installingDependencyName={installingDependencyName}
              installingDependencyTargetId={installingDependencyTargetId}
              installingCloudflared={installingCloudflared}
              startingCloudflared={startingCloudflared}
              stoppingCloudflared={stoppingCloudflared}
              onStart={onStartApiProxy}
              onStop={() => void onStopApiProxy()}
              onCreateApiProxyKey={onCreateApiProxyKey}
              onUpdateApiProxyKey={onUpdateApiProxyKey}
              onDeleteApiProxyKey={onDeleteApiProxyKey}
              onRegenerateApiProxyKey={onRegenerateApiProxyKey}
              onSelectApiProxyUsageRange={onSelectApiProxyUsageRange}
              onSelectApiProxyUsageMetric={onSelectApiProxyUsageMetric}
              onClearApiProxyUsageStats={onClearApiProxyUsageStats}
              onRefreshApiKey={() => void onRefreshApiProxyKey()}
              onBindCodexProxy={() => void onBindCodexToApiProxy()}
              onRestoreCodexProxy={() => void onRestoreCodexProxyBinding()}
              onRefresh={() => void loadApiProxyStatus()}
              onToggleAutoStart={(enabled) =>
                void updateSettings(
                  { autoStartApiProxy: enabled },
                  { silent: true, keepInteractive: true },
                )
              }
              onPersistPort={(port) =>
                updateSettings(
                  { apiProxyPort: port },
                  { silent: true, keepInteractive: true },
                )
              }
              onUpdateLoadBalanceMode={(mode) =>
                updateSettings(
                  { apiProxyLoadBalanceMode: mode },
                  { silent: true, keepInteractive: true },
                )
              }
              onUpdateSequentialFiveHourLimitPercent={(percent) =>
                updateSettings(
                  { apiProxySequentialFiveHourLimitPercent: percent },
                  { silent: true, keepInteractive: true },
                )
              }
              onUpdateApiProxyDisabledModels={(models) =>
                updateSettings(
                  { apiProxyDisabledModels: models },
                  { silent: true, keepInteractive: true },
                )
              }
              onUpdateRemoteServers={(servers) =>
                void onUpdateRemoteServers(servers)
              }
              onRefreshRemoteStatus={(server) =>
                void onRefreshRemoteProxyStatus(server)
              }
              onDeployRemote={(server) => void onDeployRemoteProxy(server)}
              onStartRemote={(server) => void onStartRemoteProxy(server)}
              onStopRemote={(server) => void onStopRemoteProxy(server)}
              onReadRemoteLogs={(server) => void onReadRemoteProxyLogs(server)}
              onPickLocalIdentityFile={() => onPickLocalIdentityFile()}
              onRefreshCloudflared={() => void loadCloudflaredStatus()}
              onInstallCloudflared={() => void onInstallCloudflared()}
              onStartCloudflared={(input) => void onStartCloudflared(input)}
              onStopCloudflared={() => void onStopCloudflared()}
            />
          ) : (
            <SettingsPanel
              themeMode={themeMode}
              onToggleTheme={toggleTheme}
              checkingUpdate={checkingUpdate}
              onCheckUpdate={() => void checkForAppUpdate(false)}
              onOpenExternalUrl={(url) => void openExternalUrl(url)}
              settings={settings}
              installedEditorApps={installedEditorApps}
              hasOpencodeDesktopApp={hasOpencodeDesktopApp}
              savingSettings={savingSettings}
              onUpdateSettings={(patch, options) =>
                void updateSettings(patch, options)
              }
            />
          )}
        </section>
      </main>
    </div>
  );
}

export default App;
