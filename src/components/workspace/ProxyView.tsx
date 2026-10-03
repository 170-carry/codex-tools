import { ApiProxyPanel } from "../ApiProxyPanel";
import type { CodexController } from "../../types/workspace";

export function ProxyView({ c }: { c: CodexController }) {
  return (
    <ApiProxyPanel
      status={c.apiProxyStatus}
      apiProxyKeys={c.apiProxyKeys}
      apiProxyKeyLogs={c.apiProxyKeyLogs}
      apiProxyKeysLoading={c.apiProxyKeysLoading}
      apiProxyUsageStats={c.apiProxyUsageStats}
      apiProxyUsageRange={c.apiProxyUsageRange}
      apiProxyUsageMetric={c.apiProxyUsageMetric}
      apiProxyUsageLoading={c.apiProxyUsageLoading}
      apiProxyUsageClearing={c.apiProxyUsageClearing}
      apiProxyUsageExporting={c.apiProxyUsageExporting}
      cloudflaredStatus={c.cloudflaredStatus}
      accountCount={c.accounts.length}
      autoStartEnabled={c.settings.autoStartApiProxy}
      savedPort={c.settings.apiProxyPort}
      loadBalanceMode={c.settings.apiProxyLoadBalanceMode}
      sequentialFiveHourLimitPercent={
        c.settings.apiProxySequentialFiveHourLimitPercent
      }
      apiProxySupportedModels={c.apiProxySupportedModels}
      apiProxyDisabledModels={c.settings.apiProxyDisabledModels}
      remoteServers={c.settings.remoteServers}
      remoteStatuses={c.remoteProxyStatuses}
      remoteLogs={c.remoteProxyLogs}
      savingSettings={c.savingSettings}
      starting={c.startingApiProxy}
      stopping={c.stoppingApiProxy}
      refreshingApiKey={c.refreshingApiProxyKey}
      bindingCodexProxy={c.bindingCodexProxy}
      restoringCodexProxy={c.restoringCodexProxy}
      savingApiProxyKey={c.savingApiProxyKey}
      refreshingRemoteId={c.refreshingRemoteProxyId}
      deployingRemoteId={c.deployingRemoteProxyId}
      startingRemoteId={c.startingRemoteProxyId}
      stoppingRemoteId={c.stoppingRemoteProxyId}
      readingRemoteLogsId={c.readingRemoteLogsId}
      installingDependencyName={c.installingDependencyName}
      installingDependencyTargetId={c.installingDependencyTargetId}
      installingCloudflared={c.installingCloudflared}
      startingCloudflared={c.startingCloudflared}
      stoppingCloudflared={c.stoppingCloudflared}
      onStart={c.onStartApiProxy}
      onStop={() => void c.onStopApiProxy()}
      onCreateApiProxyKey={c.onCreateApiProxyKey}
      onUpdateApiProxyKey={c.onUpdateApiProxyKey}
      onDeleteApiProxyKey={c.onDeleteApiProxyKey}
      onRegenerateApiProxyKey={c.onRegenerateApiProxyKey}
      onSelectApiProxyUsageRange={c.onSelectApiProxyUsageRange}
      onSelectApiProxyUsageMetric={c.onSelectApiProxyUsageMetric}
      onExportApiProxyUsage={c.onExportApiProxyUsage}
      onClearApiProxyUsageStats={c.onClearApiProxyUsageStats}
      onRefreshApiKey={() => void c.onRefreshApiProxyKey()}
      onBindCodexProxy={() => void c.onBindCodexToApiProxy()}
      onRestoreCodexProxy={() => void c.onRestoreCodexProxyBinding()}
      onRefresh={() => void c.loadApiProxyStatus()}
      onToggleAutoStart={(enabled) =>
        void c.updateSettings(
          { autoStartApiProxy: enabled },
          { silent: true, keepInteractive: true },
        )
      }
      onPersistPort={(port) =>
        c.updateSettings(
          { apiProxyPort: port },
          { silent: true, keepInteractive: true },
        )
      }
      onUpdateLoadBalanceMode={(mode) =>
        c.updateSettings(
          { apiProxyLoadBalanceMode: mode },
          { silent: true, keepInteractive: true },
        )
      }
      onUpdateSequentialFiveHourLimitPercent={(percent) =>
        c.updateSettings(
          { apiProxySequentialFiveHourLimitPercent: percent },
          { silent: true, keepInteractive: true },
        )
      }
      onUpdateApiProxyDisabledModels={(models) =>
        c.updateSettings(
          { apiProxyDisabledModels: models },
          { silent: true, keepInteractive: true },
        )
      }
      onUpdateRemoteServers={(servers) => void c.onUpdateRemoteServers(servers)}
      onRefreshRemoteStatus={(server) =>
        void c.onRefreshRemoteProxyStatus(server)
      }
      onDeployRemote={(server) => void c.onDeployRemoteProxy(server)}
      onStartRemote={(server) => void c.onStartRemoteProxy(server)}
      onStopRemote={(server) => void c.onStopRemoteProxy(server)}
      onReadRemoteLogs={(server) => void c.onReadRemoteProxyLogs(server)}
      onPickLocalIdentityFile={() => c.onPickLocalIdentityFile()}
      onRefreshCloudflared={() => void c.loadCloudflaredStatus()}
      onInstallCloudflared={() => void c.onInstallCloudflared()}
      onStartCloudflared={(input) => void c.onStartCloudflared(input)}
      onStopCloudflared={() => void c.onStopCloudflared()}
    />
  );
}
