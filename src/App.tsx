import { useState } from "react";
import "./App.css";
import { AddAccountSection } from "./components/AddAccountSection";
import { AddAccountDialog } from "./components/AddAccountDialog";
import { AccountsGrid } from "./components/AccountsGrid";
import { AppTopBar } from "./components/AppTopBar";
import { MetaStrip } from "./components/MetaStrip";
import { NoticeBanner } from "./components/NoticeBanner";
import { SettingsPanel } from "./components/SettingsPanel";
import { UpdateBanner } from "./components/UpdateBanner";
import { useCodexController } from "./hooks/useCodexController";
import { useThemeMode } from "./hooks/useThemeMode";

function App() {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const { themeMode, toggleTheme } = useThemeMode();
  const {
    accounts,
    loading,
    refreshing,
    startingAdd,
    addFlow,
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
    savingSettings,
    currentCount,
    refreshUsage,
    checkForAppUpdate,
    installPendingUpdate,
    openManualDownloadPage,
    closeUpdateDialog,
    updateSettings,
    onStartAddAccount,
    onCancelAddFlow,
    onDelete,
    onSwitch,
    onSmartSwitch,
    smartSwitching,
  } = useCodexController();

  return (
    <div className="shell">
      <div className="ambient" />
      <main className="panel">
        <AppTopBar
          onOpenSettings={() => setSettingsOpen(true)}
          onCheckUpdate={() => void checkForAppUpdate(false)}
          checkingUpdate={checkingUpdate}
          installingUpdate={installingUpdate}
          onRefresh={() => void refreshUsage(false)}
          refreshing={refreshing}
        />

        <SettingsPanel
          open={settingsOpen}
          onClose={() => setSettingsOpen(false)}
          themeMode={themeMode}
          onToggleTheme={toggleTheme}
          settings={settings}
          installedEditorApps={installedEditorApps}
          savingSettings={savingSettings}
          onUpdateSettings={(patch, options) => void updateSettings(patch, options)}
        />

        <MetaStrip accountCount={accounts.length} currentCount={currentCount} />

        <AddAccountSection
          startingAdd={startingAdd}
          addFlowActive={Boolean(addFlow)}
          onStartAddAccount={() => void onStartAddAccount()}
          onSmartSwitch={() => void onSmartSwitch()}
          smartSwitching={smartSwitching}
        />
        <AddAccountDialog
          open={startingAdd || Boolean(addFlow)}
          startingAdd={startingAdd}
          addFlowActive={Boolean(addFlow)}
          onClose={onCancelAddFlow}
        />

        <NoticeBanner notice={notice} />
        <UpdateBanner
          open={updateDialogOpen}
          pendingUpdate={pendingUpdate}
          updateProgress={updateProgress}
          installingUpdate={installingUpdate}
          onClose={closeUpdateDialog}
          onManualDownload={() => void openManualDownloadPage()}
          onRetryAutoDownload={() => void installPendingUpdate()}
        />

        <AccountsGrid
          accounts={accounts}
          loading={loading}
          switchingId={switchingId}
          pendingDeleteId={pendingDeleteId}
          onSwitch={(account) => void onSwitch(account)}
          onDelete={(account) => void onDelete(account)}
        />
      </main>
    </div>
  );
}

export default App;
