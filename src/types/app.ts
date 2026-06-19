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

export type CodexTokenTotals = {
  inputTokens: number;
  cachedInputTokens: number;
  outputTokens: number;
  reasoningOutputTokens: number;
  totalTokens: number;
};

export type CodexTokenSessionUsage = {
  startedAt: number | null;
  updatedAt: number;
  total: CodexTokenTotals;
};

export type CodexTokenUsageSnapshot = {
  updatedAt: number;
  sourcePathCount: number;
  failedPathCount: number;
  eventCount: number;
  last24h: CodexTokenTotals;
  last3d: CodexTokenTotals;
  last7d: CodexTokenTotals;
  last30d: CodexTokenTotals;
  latestSession: CodexTokenSessionUsage | null;
};

export type CodexBudgetAlert = "none" | "ok" | "warning" | "danger";

export type CodexProjectCostBreakdown = {
  projectPath: string;
  projectName: string;
  sessionCount: number;
  promptCount: number;
  eventCount: number;
  total: CodexTokenTotals;
  costUsd: number;
  lastAt: number | null;
};

export type CodexSessionCostBreakdown = {
  sessionId: string;
  parentSessionId: string | null;
  projectPath: string;
  projectName: string;
  startedAt: number | null;
  updatedAt: number | null;
  durationSeconds: number | null;
  promptCount: number;
  eventCount: number;
  model: string;
  total: CodexTokenTotals;
  costUsd: number;
  sourcePath: string;
};

export type DeleteCodexSessionResult = {
  sessionId: string;
  deletedPath: string;
};

export type CodexHourlyCostBucket = {
  weekday: number;
  hour: number;
  calls: number;
  tokens: number;
  costUsd: number;
};

export type CodexPromptCostBreakdown = {
  sessionId: string;
  projectPath: string;
  projectName: string;
  timestamp: number;
  model: string;
  promptPreview: string;
  promptChars: number;
  total: CodexTokenTotals;
  costUsd: number;
  sourcePath: string;
};

export type CodexCostAnalyticsSnapshot = {
  updatedAt: number;
  pricingSource: string;
  sourcePathCount: number;
  failedPathCount: number;
  eventCount: number;
  total: CodexTokenTotals;
  totalCostUsd: number;
  last7d: CodexTokenTotals;
  last7dCostUsd: number;
  weeklyBudgetUsd: number | null;
  weeklyBudgetPercent: number | null;
  weeklyBudgetAlert: CodexBudgetAlert;
  projects: CodexProjectCostBreakdown[];
  sessions: CodexSessionCostBreakdown[];
  heatmap: CodexHourlyCostBucket[];
  topPrompts: CodexPromptCostBreakdown[];
};

export type CodexCostAnalyticsProgress = {
  stage: "scanning" | "caching" | "complete" | string;
  processedFiles: number;
  totalFiles: number;
  percent: number;
  currentPath: string | null;
};

export type AccountSourceKind = "chatgpt" | "relay";

export type AccountSummary = {
  id: string;
  label: string;
  sourceKind: AccountSourceKind;
  email: string | null;
  accountKey: string;
  accountId: string;
  planType: string | null;
  apiBaseUrl: string | null;
  modelName: string | null;
  balanceText: string | null;
  profileAuthReady: boolean;
  profileConfigReady: boolean;
  profileIntegrityError: string | null;
  profileLastValidatedAt: number | null;
  profileLastValidationError: string | null;
  addedAt: number;
  updatedAt: number;
  usage: UsageSnapshot | null;
  usageError: string | null;
  authRefreshBlocked: boolean;
  authRefreshError: string | null;
  apiProxyEnabled: boolean;
  isCurrent: boolean;
};

export type SwitchAccountResult = {
  accountId: string;
  launchedAppPath: string | null;
  usedFallbackCli: boolean;
  opencodeSynced: boolean;
  opencodeSyncError: string | null;
  opencodeDesktopRestarted: boolean;
  opencodeDesktopRestartError: string | null;
  restartedEditorApps: EditorAppId[];
  editorRestartError: string | null;
};

export type PreparedOauthLogin = {
  authUrl: string;
  redirectUri: string;
};

export type OauthCallbackFinishedEvent = {
  result: ImportAccountsResult | null;
  error: string | null;
};

export type AuthJsonImportInput = {
  source: string;
  content: string;
  label: string | null;
};

export type CreateApiAccountInput = {
  label: string;
  baseUrl: string;
  apiKey: string;
  modelName: string;
  forceSave: boolean;
};

export type TestApiAccountConnectionInput = {
  label: string;
  baseUrl: string;
  apiKey: string;
  modelName: string;
};

export type TestApiAccountConnectionResult = {
  ok: boolean;
  balanceText: string | null;
  message: string;
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
  codexProxyBound: boolean;
  codexProxyRestoreAvailable: boolean;
  codexProxyBaseUrl: string | null;
  codexProxyConfigPath: string | null;
  activeAccountKey: string | null;
  activeAccountId: string | null;
  activeAccountLabel: string | null;
  lastError: string | null;
};

export type ApiProxyKey = {
  id: string;
  label: string;
  key: string;
  enabled: boolean;
  allowedModels: string[];
  allowedReasoningEfforts: string[];
  allowedServiceTiers: string[];
  createdAt: number;
  updatedAt: number;
};

export type CreateApiProxyKeyInput = {
  label: string;
  key: string | null;
  allowedModels: string[];
  allowedReasoningEfforts: string[];
  allowedServiceTiers: string[];
};

export type UpdateApiProxyKeyInput = {
  id: string;
  label?: string;
  enabled?: boolean;
  allowedModels?: string[];
  allowedReasoningEfforts?: string[];
  allowedServiceTiers?: string[];
};

export type ApiProxyKeyUsageLogEntry = {
  timestamp: number;
  keyId: string | null;
  keyLabel: string | null;
  model: string;
  route: string | null;
  reasoningEffort: string | null;
  serviceTier: string | null;
  calls: number;
  tokens: number;
};

export type ApiProxyUsageRange = "1h" | "24h" | "7d" | "14d" | "30d";

export type ApiProxyUsageMetric = "calls" | "tokens";

export type ApiProxyUsagePoint = {
  timestamp: number;
  calls: number;
  tokens: number;
};

export type ApiProxyUsageSeries = {
  model: string;
  totalCalls: number;
  totalTokens: number;
  points: ApiProxyUsagePoint[];
};

export type ApiProxyUsageStats = {
  updatedAt: number;
  rangeSeconds: number;
  bucketSeconds: number;
  series: ApiProxyUsageSeries[];
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
  debugPreview?: boolean;
};

export type ThemeMode = "light" | "dark";

export type TrayUsageDisplayMode = "remaining" | "used" | "hidden";

export type ApiProxyLoadBalanceMode = "average" | "sequential";

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
  smartSwitchIncludeApi: boolean;
  launchCodexAsAdmin: boolean;
  codexLaunchPath: string | null;
  syncOpencodeOpenaiAuth: boolean;
  restartOpencodeDesktopOnSwitch: boolean;
  restartEditorsOnSwitch: boolean;
  restartEditorTargets: EditorAppId[];
  autoStartApiProxy: boolean;
  apiProxyPort: number;
  apiProxyLoadBalanceMode: ApiProxyLoadBalanceMode;
  apiProxySequentialFiveHourLimitPercent: number;
  apiProxyDisabledModels: string[];
  codexAnalyticsWeeklyBudgetUsd: number | null;
  remoteServers: RemoteServerConfig[];
  locale: AppLocale;
  skippedUpdateVersion: string | null;
};

export type UpdateSettingsOptions = {
  silent?: boolean;
  keepInteractive?: boolean;
};
