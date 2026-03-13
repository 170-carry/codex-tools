import enUsRaw from "./locales/en-US.json";
import jaJpRaw from "./locales/ja-JP.json";
import koKrRaw from "./locales/ko-KR.json";
import ruRuRaw from "./locales/ru-RU.json";
import zhCnRaw from "./locales/zh-CN.json";

export const SUPPORTED_LOCALES = ["zh-CN", "en-US", "ja-JP", "ko-KR", "ru-RU"] as const;

export type AppLocale = (typeof SUPPORTED_LOCALES)[number];

export type LocaleOption = {
  code: AppLocale;
  shortLabel: string;
  nativeLabel: string;
};

export const LOCALE_OPTIONS: LocaleOption[] = [
  { code: "zh-CN", shortLabel: "中", nativeLabel: "中文" },
  { code: "en-US", shortLabel: "EN", nativeLabel: "English" },
  { code: "ja-JP", shortLabel: "日", nativeLabel: "日本語" },
  { code: "ko-KR", shortLabel: "한", nativeLabel: "한국어" },
  { code: "ru-RU", shortLabel: "RU", nativeLabel: "Русский" },
];

export const DEFAULT_LOCALE: AppLocale = "zh-CN";

export function isSupportedLocale(value: string | null | undefined): value is AppLocale {
  return (
    value === "zh-CN" ||
    value === "en-US" ||
    value === "ja-JP" ||
    value === "ko-KR" ||
    value === "ru-RU"
  );
}

export function getNextLocale(current: AppLocale): AppLocale {
  const index = LOCALE_OPTIONS.findIndex((item) => item.code === current);
  if (index < 0) {
    return DEFAULT_LOCALE;
  }
  return LOCALE_OPTIONS[(index + 1) % LOCALE_OPTIONS.length].code;
}

export type MessageCatalog = {
  common: {
    close: string;
  };
  topBar: {
    appTitle: string;
    logoAlt: string;
    checkUpdate: string;
    checkingUpdate: string;
    manualRefresh: string;
    refreshing: string;
    openSettings: string;
    toggleLanguage: (nextLanguage: string) => string;
    languagePicker: string;
  };
  metaStrip: {
    ariaLabel: string;
    accountCount: string;
    currentActive: string;
  };
  addAccount: {
    smartSwitch: string;
    startButton: string;
    startingButton: string;
    waitingButton: string;
    dialogAriaLabel: string;
    dialogTitle: string;
    dialogSubtitle: string;
    launchingTitle: string;
    watchingTitle: string;
    launchingDetail: string;
    watchingDetail: string;
    cancelListening: string;
    closeDialog: string;
    tabsAriaLabel: string;
    oauthTab: string;
    uploadTab: string;
    oauthStart: string;
    uploadChooseFiles: string;
    uploadChooseFolder: string;
    uploadNoJsonFiles: string;
    uploadFileSummary: (firstPath: string, count: number) => string;
    uploadSelectedCount: (count: number) => string;
    uploadNoFiles: string;
    uploadImporting: string;
    uploadStartImport: string;
  };
  accountCard: {
    currentStamp: string;
    launch: string;
    launching: string;
    delete: string;
    deleteConfirm: string;
    used: string;
    remaining: string;
    resetAt: string;
    credits: string;
    unlimited: string;
    fiveHourFallback: string;
    oneWeekFallback: string;
    oneWeekLabel: string;
    hourSuffix: string;
    minuteSuffix: string;
    planLabels: Record<string, string>;
  };
  accountsGrid: {
    emptyTitle: string;
    emptyDescription: string;
  };
  bottomDock: {
    ariaLabel: string;
    accounts: string;
    proxy: string;
    settings: string;
  };
  apiProxy: {
    kicker: string;
    title: string;
    hint: string;
    statusLabel: string;
    statusRunning: string;
    statusStopped: string;
    portLabel: string;
    accountCountLabel: string;
    defaultStartLabel: string;
    defaultStartEnabled: string;
    defaultStartDisabled: string;
    portInputAriaLabel: string;
    refreshStatus: string;
    stop: string;
    stopping: string;
    start: string;
    starting: string;
    baseUrlLabel: string;
    localBaseUrlLabel: string;
    lanBaseUrlLabel: string;
    copy: string;
    baseUrlPlaceholder: string;
    apiKeyLabel: string;
    refreshKey: string;
    refreshingKey: string;
    apiKeyPlaceholder: string;
    activeAccountLabel: string;
    activeAccountEmptyTitle: string;
    activeAccountEmptyDescription: string;
    lastErrorLabel: string;
    none: string;
    remoteKicker: string;
    remoteTitle: string;
    remoteDescription: string;
    remoteAddServer: string;
    remoteExpand: string;
    remoteCollapse: string;
    remoteEmptyTitle: string;
    remoteEmptyDescription: string;
    remoteNameLabel: string;
    remoteHostLabel: string;
    remoteSshPortLabel: string;
    remoteUserLabel: string;
    remoteAuthLabel: string;
    remoteIdentityFileLabel: string;
    remoteIdentityFilePlaceholder: string;
    remotePickIdentityFile: string;
    remoteDirLabel: string;
    remoteListenPortLabel: string;
    remoteAuthKeyContent: string;
    remoteAuthKeyFile: string;
    remoteAuthKeyPath: string;
    remoteAuthPassword: string;
    remotePrivateKeyLabel: string;
    remotePrivateKeyPlaceholder: string;
    remotePasswordLabel: string;
    remotePasswordPlaceholder: string;
    remoteSave: string;
    remoteRemove: string;
    remoteDeploy: string;
    remoteDeploying: string;
    remoteRefresh: string;
    remoteRefreshing: string;
    remoteStart: string;
    remoteStarting: string;
    remoteStop: string;
    remoteStopping: string;
    remoteInstalledLabel: string;
    remoteInstalledYes: string;
    remoteInstalledNo: string;
    remoteSystemdLabel: string;
    remoteEnabledLabel: string;
    remoteRunningLabel: string;
    remotePidLabel: string;
    remoteServiceLabel: string;
    remoteBaseUrlLabel: string;
    remoteApiKeyLabel: string;
    remoteLogsLabel: string;
    remoteLogsEmpty: string;
    remoteReadLogs: string;
    remoteReadingLogs: string;
    remoteLastErrorLabel: string;
    remoteStatusUnknown: string;
    remoteDeployProgressTitle: (label: string) => string;
    remoteDeployStageValidating: string;
    remoteDeployStageDetectingPlatform: string;
    remoteDeployStagePreparingBuilder: string;
    remoteDeployStageBuildingBinary: string;
    remoteDeployStagePreparingFiles: string;
    remoteDeployStageUploadingBinary: string;
    remoteDeployStageUploadingAccounts: string;
    remoteDeployStageUploadingService: string;
    remoteDeployStageInstallingService: string;
    remoteDeployStageVerifying: string;
    cloudflaredKicker: string;
    cloudflaredTitle: string;
    cloudflaredDescription: string;
    cloudflaredToggle: string;
    startLocalProxyFirstTitle: string;
    startLocalProxyFirstDescription: string;
    notInstalledLabel: string;
    installTitle: string;
    installDescription: string;
    installing: string;
    installButton: string;
    quickModeLabel: string;
    quickModeTitle: string;
    quickModeDescription: string;
    namedModeLabel: string;
    namedModeTitle: string;
    namedModeDescription: string;
    quickNoteTitle: string;
    quickNoteBody: string;
    apiTokenLabel: string;
    apiTokenPlaceholder: string;
    accountIdLabel: string;
    accountIdPlaceholder: string;
    zoneIdLabel: string;
    zoneIdPlaceholder: string;
    hostnameLabel: string;
    hostnamePlaceholder: string;
    useHttp2: string;
    refreshPublicStatus: string;
    stopPublic: string;
    stoppingPublic: string;
    startPublic: string;
    startingPublic: string;
    publicStatusLabel: string;
    publicStatusRunning: string;
    publicStatusStopped: string;
    publicStatusRunningDescription: string;
    publicStatusStoppedDescription: string;
    publicUrlLabel: string;
    installPathLabel: string;
    notDetected: string;
  };
  settings: {
    dialogAriaLabel: string;
    title: string;
    subtitle: string;
    languageSubtitle: string;
    close: string;
    launchAtStartup: {
      label: string;
      description: string;
      checkedText: string;
      uncheckedText: string;
    };
    launchCodexAfterSwitch: {
      label: string;
      description: string;
      checkedText: string;
      uncheckedText: string;
    };
    syncOpencode: {
      label: string;
      description: string;
      checkedText: string;
      uncheckedText: string;
    };
    restartEditorsOnSwitch: {
      label: string;
      description: string;
      checkedText: string;
      uncheckedText: string;
    };
    restartEditorTargets: {
      label: string;
      description: string;
    };
    noSupportedEditors: string;
    trayUsageDisplay: {
      label: string;
      description: string;
      groupAriaLabel: string;
      remaining: string;
      used: string;
      hidden: string;
    };
    theme: {
      label: string;
      description: string;
      switchAriaLabel: string;
      dark: string;
      light: string;
    };
  };
  editorPicker: {
    ariaLabel: string;
    placeholder: string;
  };
  editorAppLabels: Record<string, string>;
  updateDialog: {
    ariaLabel: string;
    title: (version: string) => string;
    subtitle: (currentVersion: string) => string;
    close: string;
    publishedAt: (date: string) => string;
    autoDownloading: string;
    autoPaused: string;
    manualDownload: string;
    retryAutoDownload: string;
    retryingAutoDownload: string;
  };
  notices: {
    settingsUpdated: string;
    updateSettingsFailed: (error: string) => string;
    usageRefreshed: string;
    refreshFailed: (error: string) => string;
    restoreAuthFailed: (error: string) => string;
    preparingUpdateDownload: string;
    alreadyLatest: string;
    updateDownloadStarted: string;
    updateDownloadingPercent: (percent: number) => string;
    updateDownloading: string;
    updateDownloadFinished: string;
    updateInstalling: string;
    updateInstallFailed: (error: string) => string;
    foundNewVersion: (version: string, currentVersion: string) => string;
    updateCheckFailed: (error: string) => string;
    openManualDownloadFailed: (error: string) => string;
    addAccountSuccess: string;
    addAccountAutoImportFailed: (error: string) => string;
    addAccountTimeout: string;
    startLoginFlowFailed: (error: string) => string;
    deleteConfirm: (label: string) => string;
    accountDeleted: string;
    deleteFailed: (error: string) => string;
    switchedOnly: string;
    switchedAndLaunchByCli: string;
    switchedAndLaunching: string;
    opencodeSyncFailed: (base: string, error: string) => string;
    opencodeSynced: (base: string) => string;
    editorRestartFailed: (base: string, error: string) => string;
    editorsRestarted: (base: string, labels: string) => string;
    noEditorRestarted: (base: string) => string;
    switchFailed: (error: string) => string;
    smartSwitchNoTarget: string;
    smartSwitchAlreadyBest: string;
    fileImportPrefix: string;
    importFilesRequired: string;
    importFailedPlain: (prefix: string, error: string) => string;
    importFailedWithSource: (prefix: string, source: string, error: string) => string;
    importFailedNoValidJson: (prefix: string) => string;
    importSummaryAdded: (count: number) => string;
    importSummaryUpdated: (count: number) => string;
    importSummaryFailed: (count: number) => string;
    importSummaryFirstFailure: (source: string, error: string) => string;
    importSummaryDone: (prefix: string, summary: string, suffix: string) => string;
    proxyLocalTargetFallback: string;
    proxyStarted: (target: string) => string;
    proxyStartFailed: (error: string) => string;
    proxyStopped: string;
    proxyStopFailed: (error: string) => string;
    proxyKeyRefreshed: string;
    proxyKeyRefreshFailed: (error: string) => string;
    installingDependency: (name: string) => string;
    dependencyInstalled: (name: string) => string;
    dependencyInstallFailed: (name: string, error: string) => string;
    remoteStatusFailed: (label: string, error: string) => string;
    remoteProxyDeployed: (label: string) => string;
    remoteProxyDeployFailed: (label: string, error: string) => string;
    remoteProxyStarted: (label: string) => string;
    remoteProxyStartFailed: (label: string, error: string) => string;
    remoteProxyStopped: (label: string) => string;
    remoteProxyStopFailed: (label: string, error: string) => string;
    remoteLogsFailed: (label: string, error: string) => string;
    pickIdentityFileFailed: (error: string) => string;
    cloudflaredInstalled: string;
    cloudflaredInstallFailed: (error: string) => string;
    cloudflaredPublicUrlFallback: string;
    cloudflaredStarted: (target: string) => string;
    cloudflaredStartFailed: (error: string) => string;
    cloudflaredStopped: string;
    cloudflaredStopFailed: (error: string) => string;
  };
};

type Rawify<T> = T extends (...args: infer _Args) => string
  ? string
  : T extends Record<string, unknown>
    ? { [K in keyof T]: Rawify<T[K]> }
    : T;

type RawMessageCatalog = Rawify<MessageCatalog>;

function fillTemplate(template: string, values: Record<string, string | number>): string {
  return template.replace(/\{\{\s*([a-zA-Z0-9_]+)\s*\}\}/g, (_, key: string) => {
    const value = values[key];
    return value === undefined ? "" : String(value);
  });
}

function compileLocale(raw: RawMessageCatalog): MessageCatalog {
  return {
    common: raw.common,
    topBar: {
      ...raw.topBar,
      toggleLanguage: (nextLanguage) => fillTemplate(raw.topBar.toggleLanguage, { nextLanguage }),
    },
    metaStrip: raw.metaStrip,
    addAccount: {
      ...raw.addAccount,
      uploadFileSummary: (firstPath, count) =>
        fillTemplate(raw.addAccount.uploadFileSummary, {
          firstPath,
          count,
          remainingCount: Math.max(count - 1, 0),
        }),
      uploadSelectedCount: (count) => fillTemplate(raw.addAccount.uploadSelectedCount, { count }),
    },
    accountCard: raw.accountCard,
    accountsGrid: raw.accountsGrid,
    bottomDock: raw.bottomDock,
    apiProxy: {
      ...raw.apiProxy,
      remoteDeployProgressTitle: (label) =>
        fillTemplate(raw.apiProxy.remoteDeployProgressTitle, { label }),
    },
    settings: raw.settings,
    editorPicker: raw.editorPicker,
    editorAppLabels: raw.editorAppLabels,
    updateDialog: {
      ...raw.updateDialog,
      title: (version) => fillTemplate(raw.updateDialog.title, { version }),
      subtitle: (currentVersion) =>
        fillTemplate(raw.updateDialog.subtitle, { currentVersion }),
      publishedAt: (date) => fillTemplate(raw.updateDialog.publishedAt, { date }),
    },
    notices: {
      ...raw.notices,
      updateSettingsFailed: (error) => fillTemplate(raw.notices.updateSettingsFailed, { error }),
      refreshFailed: (error) => fillTemplate(raw.notices.refreshFailed, { error }),
      restoreAuthFailed: (error) => fillTemplate(raw.notices.restoreAuthFailed, { error }),
      updateDownloadingPercent: (percent) =>
        fillTemplate(raw.notices.updateDownloadingPercent, { percent }),
      updateInstallFailed: (error) => fillTemplate(raw.notices.updateInstallFailed, { error }),
      foundNewVersion: (version, currentVersion) =>
        fillTemplate(raw.notices.foundNewVersion, { version, currentVersion }),
      updateCheckFailed: (error) => fillTemplate(raw.notices.updateCheckFailed, { error }),
      openManualDownloadFailed: (error) =>
        fillTemplate(raw.notices.openManualDownloadFailed, { error }),
      addAccountAutoImportFailed: (error) =>
        fillTemplate(raw.notices.addAccountAutoImportFailed, { error }),
      startLoginFlowFailed: (error) => fillTemplate(raw.notices.startLoginFlowFailed, { error }),
      deleteConfirm: (label) => fillTemplate(raw.notices.deleteConfirm, { label }),
      deleteFailed: (error) => fillTemplate(raw.notices.deleteFailed, { error }),
      opencodeSyncFailed: (base, error) =>
        fillTemplate(raw.notices.opencodeSyncFailed, { base, error }),
      opencodeSynced: (base) => fillTemplate(raw.notices.opencodeSynced, { base }),
      editorRestartFailed: (base, error) =>
        fillTemplate(raw.notices.editorRestartFailed, { base, error }),
      editorsRestarted: (base, labels) =>
        fillTemplate(raw.notices.editorsRestarted, { base, labels }),
      noEditorRestarted: (base) => fillTemplate(raw.notices.noEditorRestarted, { base }),
      switchFailed: (error) => fillTemplate(raw.notices.switchFailed, { error }),
      importFailedPlain: (prefix, error) =>
        fillTemplate(raw.notices.importFailedPlain, { prefix, error }),
      importFailedWithSource: (prefix, source, error) =>
        fillTemplate(raw.notices.importFailedWithSource, { prefix, source, error }),
      importFailedNoValidJson: (prefix) =>
        fillTemplate(raw.notices.importFailedNoValidJson, { prefix }),
      importSummaryAdded: (count) => fillTemplate(raw.notices.importSummaryAdded, { count }),
      importSummaryUpdated: (count) => fillTemplate(raw.notices.importSummaryUpdated, { count }),
      importSummaryFailed: (count) => fillTemplate(raw.notices.importSummaryFailed, { count }),
      importSummaryFirstFailure: (source, error) =>
        fillTemplate(raw.notices.importSummaryFirstFailure, { source, error }),
      importSummaryDone: (prefix, summary, suffix) =>
        fillTemplate(raw.notices.importSummaryDone, { prefix, summary, suffix }).trim(),
      proxyStarted: (target) => fillTemplate(raw.notices.proxyStarted, { target }),
      proxyStartFailed: (error) => fillTemplate(raw.notices.proxyStartFailed, { error }),
      proxyStopFailed: (error) => fillTemplate(raw.notices.proxyStopFailed, { error }),
      proxyKeyRefreshFailed: (error) =>
        fillTemplate(raw.notices.proxyKeyRefreshFailed, { error }),
      installingDependency: (name) =>
        fillTemplate(raw.notices.installingDependency, { name }),
      dependencyInstalled: (name) =>
        fillTemplate(raw.notices.dependencyInstalled, { name }),
      dependencyInstallFailed: (name, error) =>
        fillTemplate(raw.notices.dependencyInstallFailed, { name, error }),
      remoteStatusFailed: (label, error) =>
        fillTemplate(raw.notices.remoteStatusFailed, { label, error }),
      remoteProxyDeployed: (label) => fillTemplate(raw.notices.remoteProxyDeployed, { label }),
      remoteProxyDeployFailed: (label, error) =>
        fillTemplate(raw.notices.remoteProxyDeployFailed, { label, error }),
      remoteProxyStarted: (label) => fillTemplate(raw.notices.remoteProxyStarted, { label }),
      remoteProxyStartFailed: (label, error) =>
        fillTemplate(raw.notices.remoteProxyStartFailed, { label, error }),
      remoteProxyStopped: (label) => fillTemplate(raw.notices.remoteProxyStopped, { label }),
      remoteProxyStopFailed: (label, error) =>
        fillTemplate(raw.notices.remoteProxyStopFailed, { label, error }),
      remoteLogsFailed: (label, error) =>
        fillTemplate(raw.notices.remoteLogsFailed, { label, error }),
      pickIdentityFileFailed: (error) =>
        fillTemplate(raw.notices.pickIdentityFileFailed, { error }),
      cloudflaredInstallFailed: (error) =>
        fillTemplate(raw.notices.cloudflaredInstallFailed, { error }),
      cloudflaredStarted: (target) => fillTemplate(raw.notices.cloudflaredStarted, { target }),
      cloudflaredStartFailed: (error) =>
        fillTemplate(raw.notices.cloudflaredStartFailed, { error }),
      cloudflaredStopFailed: (error) =>
        fillTemplate(raw.notices.cloudflaredStopFailed, { error }),
    },
  };
}

export const MESSAGES: Record<AppLocale, MessageCatalog> = {
  "zh-CN": compileLocale(zhCnRaw as RawMessageCatalog),
  "en-US": compileLocale(enUsRaw as RawMessageCatalog),
  "ja-JP": compileLocale(jaJpRaw as RawMessageCatalog),
  "ko-KR": compileLocale(koKrRaw as RawMessageCatalog),
  "ru-RU": compileLocale(ruRuRaw as RawMessageCatalog),
};
