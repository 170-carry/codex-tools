import type { AppLocale } from "../i18n/catalog";

export type UsageWindow = {
  usedPercent: number;
  windowSeconds: number;
  resetAt: number | null;
};

export type CreditSnapshot = {
  hasCredits: boolean;
  unlimited: boolean;
  balance: string | null;
};

export type UsageSnapshot = {
  fetchedAt: number;
  planType: string | null;
  fiveHour: UsageWindow | null;
  oneWeek: UsageWindow | null;
  credits: CreditSnapshot | null;
};

export type AccountSummary = {
  id: string;
  label: string;
  email: string | null;
  accountId: string;
  planType: string | null;
  addedAt: number;
  updatedAt: number;
  usage: UsageSnapshot | null;
  usageError: string | null;
  isCurrent: boolean;
};

export type SwitchAccountResult = {
  accountId: string;
  launchedAppPath: string | null;
  usedFallbackCli: boolean;
  opencodeSynced: boolean;
  opencodeSyncError: string | null;
  restartedEditorApps: EditorAppId[];
  editorRestartError: string | null;
};

export type CurrentAuthStatus = {
  available: boolean;
  accountId: string | null;
  email: string | null;
  planType: string | null;
  authMode: string | null;
  lastRefresh: string | null;
  fileModifiedAt: number | null;
  fingerprint: string | null;
};

export type AuthJsonImportInput = {
  source: string;
  content: string;
  label: string | null;
};

export type ImportAccountFailure = {
  source: string;
  error: string;
};

export type ImportAccountsResult = {
  totalCount: number;
  importedCount: number;
  updatedCount: number;
  failures: ImportAccountFailure[];
};

export type ApiProxyStatus = {
  running: boolean;
  port: number | null;
  apiKey: string | null;
  baseUrl: string | null;
  lanBaseUrl: string | null;
  activeAccountId: string | null;
  activeAccountLabel: string | null;
  lastError: string | null;
};

export type RemoteAuthMode = "keyContent" | "keyFile" | "keyPath" | "password";

export type RemoteServerConfig = {
  id: string;
  label: string;
  host: string;
  sshPort: number;
  sshUser: string;
  authMode: RemoteAuthMode;
  identityFile: string | null;
  privateKey: string | null;
  password: string | null;
  remoteDir: string;
  listenPort: number;
};

export type RemoteProxyStatus = {
  installed: boolean;
  serviceInstalled: boolean;
  running: boolean;
  enabled: boolean;
  serviceName: string;
  pid: number | null;
  baseUrl: string;
  apiKey: string | null;
  lastError: string | null;
};

export type RemoteDeployStage =
  | "validating"
  | "detectingPlatform"
  | "preparingBuilder"
  | "buildingBinary"
  | "preparingFiles"
  | "uploadingBinary"
  | "uploadingAccounts"
  | "uploadingService"
  | "installingService"
  | "verifying";

export type RemoteDeployProgress = {
  serverId: string;
  label: string;
  stage: RemoteDeployStage;
  progress: number;
  detail: string | null;
};

export type CloudflaredTunnelMode = "quick" | "named";

export type CloudflaredStatus = {
  installed: boolean;
  binaryPath: string | null;
  running: boolean;
  tunnelMode: CloudflaredTunnelMode | null;
  publicUrl: string | null;
  customHostname: string | null;
  useHttp2: boolean;
  lastError: string | null;
};

export type NamedCloudflaredTunnelInput = {
  apiToken: string;
  accountId: string;
  zoneId: string;
  hostname: string;
};

export type StartCloudflaredTunnelInput = {
  apiProxyPort: number;
  useHttp2: boolean;
  mode: CloudflaredTunnelMode;
  named: NamedCloudflaredTunnelInput | null;
};

export type Notice = {
  type: "ok" | "error" | "info";
  message: string;
};

export type PendingUpdateInfo = {
  currentVersion: string;
  version: string;
  body?: string;
  date?: string;
};

export type AddFlow = {
  baselineFingerprint: string | null;
};

export type ThemeMode = "light" | "dark";

export type TrayUsageDisplayMode = "remaining" | "used" | "hidden";

export type EditorAppId =
  | "vscode"
  | "vscodeInsiders"
  | "cursor"
  | "antigravity"
  | "kiro"
  | "trae"
  | "qoder";

export type InstalledEditorApp = {
  id: EditorAppId;
  label: string;
};

export type AppSettings = {
  launchAtStartup: boolean;
  trayUsageDisplayMode: TrayUsageDisplayMode;
  launchCodexAfterSwitch: boolean;
  syncOpencodeOpenaiAuth: boolean;
  restartEditorsOnSwitch: boolean;
  restartEditorTargets: EditorAppId[];
  autoStartApiProxy: boolean;
  apiProxyPort: number;
  remoteServers: RemoteServerConfig[];
  locale: AppLocale;
};

export type UpdateSettingsOptions = {
  silent?: boolean;
  keepInteractive?: boolean;
};
