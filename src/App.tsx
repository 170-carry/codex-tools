import { useEffect, useState } from "react";
import "./App.css";
import { ApiProxyPanel } from "./components/ApiProxyPanel";
import { AddAccountSection } from "./components/AddAccountSection";
import { AddAccountDialog } from "./components/AddAccountDialog";
import { AccountsGrid } from "./components/AccountsGrid";
import { AppTopBar } from "./components/AppTopBar";
import { BottomDock } from "./components/BottomDock";
import { MetaStrip } from "./components/MetaStrip";
import { NoticeBanner } from "./components/NoticeBanner";
import { RemoteDeployProgressToast } from "./components/RemoteDeployProgressToast";
import { SettingsPanel } from "./components/SettingsPanel";
import { UpdateBanner } from "./components/UpdateBanner";
import { useCodexController } from "./hooks/useCodexController";
import { useThemeMode } from "./hooks/useThemeMode";

type AppTab = "accounts" | "proxy" | "settings";

function App() {
  const [activeTab, setActiveTab] = useState<AppTab>("accounts");
  const { themeMode, toggleTheme } = useThemeMode();
  const {
    accounts,
    loading,
    refreshing,
    addDialogOpen,
    startingAdd,
    addFlow,
    importingUpload,
    exportingAccounts,
    switchingId,
    pendingDeleteId,
    checkingUpdate,
    installingUpdate,
    updateProgress,
    pendingUpdate,
    updateDialogOpen,
    notice,
    settings,
    installedEditorApps,
    hasOpencodeDesktopApp,
    savingSettings,
    apiProxyStatus,
    cloudflaredStatus,
    remoteProxyStatuses,
    remoteProxyLogs,
    remoteDeployProgress,
    startingApiProxy,
    stoppingApiProxy,
    refreshingApiProxyKey,
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
    checkForAppUpdate,
    installPendingUpdate,
    openManualDownloadPage,
    closeUpdateDialog,
    updateSettings,
    onOpenAddDialog,
    onStartAddAccount,
    onCloseAddDialog,
    onImportAuthFiles,
    onExportAccounts,
    loadApiProxyStatus,
    onStartApiProxy,
    onStopApiProxy,
    onRefreshApiProxyKey,
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
    };

    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [refreshUsage]);

  return (
    <div className="shell">
      <div className="ambient" />
      <main className="panel">
        <AppTopBar
          onRefresh={() => void refreshUsage(false)}
          refreshing={refreshing}
          onGoHome={() => setActiveTab("accounts")}
          showRefresh={activeTab === "accounts"}
        />

        <AddAccountDialog
          open={addDialogOpen}
          startingAdd={startingAdd}
          addFlowActive={Boolean(addFlow)}
          importingUpload={importingUpload}
          onStartOauth={onStartAddAccount}
          onImportFiles={onImportAuthFiles}
          onClose={onCloseAddDialog}
        />

        <NoticeBanner notice={notice} />
        <RemoteDeployProgressToast progress={remoteDeployProgress} />
        <UpdateBanner
          open={updateDialogOpen}
          pendingUpdate={pendingUpdate}
          updateProgress={updateProgress}
          installingUpdate={installingUpdate}
          onClose={closeUpdateDialog}
          onManualDownload={() => void openManualDownloadPage()}
          onRetryAutoDownload={() => void installPendingUpdate()}
        />

        <section className="viewStage">
          {activeTab === "accounts" ? (
            <div className="accountsPage">
              <div className="accountsHero">
                <MetaStrip
                  accountCount={accounts.length}
                  exportingAccounts={exportingAccounts}
                  onExportAccounts={() => void onExportAccounts()}
                />
                <AddAccountSection
                  startingAdd={startingAdd}
                  addFlowActive={Boolean(addFlow)}
                  onOpenAddDialog={onOpenAddDialog}
                  onSmartSwitch={() => void onSmartSwitch()}
                  smartSwitching={smartSwitching}
                />
              </div>
              <AccountsGrid
                accounts={accounts}
                loading={loading}
                switchingId={switchingId}
                pendingDeleteId={pendingDeleteId}
                onSwitch={(account) => void onSwitch(account)}
                onDelete={(account) => void onDelete(account)}
              />
            </div>
          ) : activeTab === "proxy" ? (
            <ApiProxyPanel
              status={apiProxyStatus}
              cloudflaredStatus={cloudflaredStatus}
              accountCount={accounts.length}
              autoStartEnabled={settings.autoStartApiProxy}
              savedPort={settings.apiProxyPort}
              remoteServers={settings.remoteServers}
              remoteStatuses={remoteProxyStatuses}
              remoteLogs={remoteProxyLogs}
              savingSettings={savingSettings}
              starting={startingApiProxy}
              stopping={stoppingApiProxy}
              refreshingApiKey={refreshingApiProxyKey}
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
              onRefreshApiKey={() => void onRefreshApiProxyKey()}
              onRefresh={() => void loadApiProxyStatus()}
              onToggleAutoStart={(enabled) =>
                void updateSettings(
                  { autoStartApiProxy: enabled },
                  { silent: true, keepInteractive: true },
                )}
              onPersistPort={(port) =>
                updateSettings(
                  { apiProxyPort: port },
                  { silent: true, keepInteractive: true },
                )}
              onUpdateRemoteServers={(servers) => void onUpdateRemoteServers(servers)}
              onRefreshRemoteStatus={(server) => void onRefreshRemoteProxyStatus(server)}
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
              settings={settings}
              installedEditorApps={installedEditorApps}
              hasOpencodeDesktopApp={hasOpencodeDesktopApp}
              savingSettings={savingSettings}
              onUpdateSettings={(patch, options) => void updateSettings(patch, options)}
            />
          )}
        </section>
        <BottomDock
          activeTab={activeTab}
          onSelectTab={setActiveTab}
        />
      </main>
    </div>
  );
}

export default App;
