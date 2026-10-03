import type {
  ApiProxyStatus,
  ApiProxyKey,
  ApiProxyKeyUsageLogEntry,
  ApiProxyUsageMetric,
  ApiProxyUsageRange,
  ApiProxyUsageStats,
  CloudflaredStatus,
  ApiProxyLoadBalanceMode,
  RemoteAuthMode,
  RemoteProxyStatus,
  RemoteServerConfig,
  CreateApiProxyKeyInput,
  UpdateApiProxyKeyInput,
  StartCloudflaredTunnelInput,
} from "../../types/app";
export type RemoteServerDraft = {
  id: string;
  label: string;
  host: string;
  sshPort: string;
  sshUser: string;
  authMode: RemoteAuthMode;
  identityFile: string;
  privateKey: string;
  password: string;
  remoteDir: string;
  listenPort: string;
};

export type ApiProxyPanelProps = {
  status: ApiProxyStatus;
  apiProxyKeys: ApiProxyKey[];
  apiProxyKeyLogs: ApiProxyKeyUsageLogEntry[];
  apiProxyKeysLoading: boolean;
  apiProxyUsageStats: ApiProxyUsageStats | null;
  apiProxyUsageRange: ApiProxyUsageRange;
  apiProxyUsageMetric: ApiProxyUsageMetric;
  apiProxyUsageLoading: boolean;
  apiProxyUsageClearing: boolean;
  apiProxyUsageExporting: boolean;
  cloudflaredStatus: CloudflaredStatus;
  accountCount: number;
  autoStartEnabled: boolean;
  savedPort: number;
  loadBalanceMode: ApiProxyLoadBalanceMode;
  sequentialFiveHourLimitPercent: number;
  apiProxySupportedModels: string[];
  apiProxyDisabledModels: string[];
  remoteServers: RemoteServerConfig[];
  remoteStatuses: Record<string, RemoteProxyStatus>;
  remoteLogs: Record<string, string>;
  savingSettings: boolean;
  starting: boolean;
  stopping: boolean;
  refreshingApiKey: boolean;
  bindingCodexProxy: boolean;
  restoringCodexProxy: boolean;
  savingApiProxyKey: boolean;
  refreshingRemoteId: string | null;
  deployingRemoteId: string | null;
  startingRemoteId: string | null;
  stoppingRemoteId: string | null;
  readingRemoteLogsId: string | null;
  installingDependencyName: string | null;
  installingDependencyTargetId: string | null;
  installingCloudflared: boolean;
  startingCloudflared: boolean;
  stoppingCloudflared: boolean;
  onStart: (port: number | null) => Promise<void> | void;
  onStop: () => void;
  onCreateApiProxyKey: (input: CreateApiProxyKeyInput) => Promise<void> | void;
  onUpdateApiProxyKey: (input: UpdateApiProxyKeyInput) => Promise<void> | void;
  onDeleteApiProxyKey: (id: string) => Promise<void> | void;
  onRegenerateApiProxyKey: (id: string) => Promise<void> | void;
  onSelectApiProxyUsageRange: (range: ApiProxyUsageRange) => void;
  onSelectApiProxyUsageMetric: (metric: ApiProxyUsageMetric) => void;
  onExportApiProxyUsage: (keyId: string | null) => Promise<void> | void;
  onClearApiProxyUsageStats: () => void;
  onRefreshApiKey: () => void;
  onBindCodexProxy: () => void;
  onRestoreCodexProxy: () => void;
  onRefresh: () => void;
  onToggleAutoStart: (enabled: boolean) => void;
  onPersistPort: (port: number) => Promise<void> | void;
  onUpdateLoadBalanceMode: (
    mode: ApiProxyLoadBalanceMode,
  ) => Promise<void> | void;
  onUpdateSequentialFiveHourLimitPercent: (
    percent: number,
  ) => Promise<void> | void;
  onUpdateApiProxyDisabledModels: (models: string[]) => Promise<void> | void;
  onUpdateRemoteServers: (servers: RemoteServerConfig[]) => void;
  onRefreshRemoteStatus: (server: RemoteServerConfig) => void;
  onDeployRemote: (server: RemoteServerConfig) => void;
  onStartRemote: (server: RemoteServerConfig) => void;
  onStopRemote: (server: RemoteServerConfig) => void;
  onReadRemoteLogs: (server: RemoteServerConfig) => void;
  onPickLocalIdentityFile: () => Promise<string | null>;
  onRefreshCloudflared: () => void;
  onInstallCloudflared: () => void;
  onStartCloudflared: (input: StartCloudflaredTunnelInput) => void;
  onStopCloudflared: () => void;
};
