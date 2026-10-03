/* eslint-disable react-refresh/only-export-components -- Node SSR test entry; no Fast Refresh runtime. */
import assert from "node:assert/strict";
import { renderToStaticMarkup } from "react-dom/server";
import type { ReactNode } from "react";
import { MESSAGES } from "../../src/i18n/catalog";
import { I18nProvider } from "../../src/i18n/I18nProvider";
import { AnalyticsPanel } from "../../src/components/AnalyticsPanel";
import { ApiProxyPanel } from "../../src/components/ApiProxyPanel";
import { SettingsPanel } from "../../src/components/SettingsPanel";
import { PageSections } from "../../src/components/workspace/PageSections";
import { QuotaSettings } from "../../src/components/settings/QuotaSettings";
import { useSettingsWorkspace } from "../../src/components/settings/useSettingsWorkspace";
import type { SettingsPanelProps } from "../../src/components/settings/types";
import { TokenUsageStrip } from "../../src/components/accounts/TokenUsageStrip";
import { getUiCopy } from "../../src/components/accounts/accountCopy";
import { accounts, analytics, tokenUsage, now } from "./fixtures";
import {
  DEFAULT_SETTINGS,
  DEFAULT_API_PROXY_STATUS,
  DEFAULT_CLOUDFLARED_STATUS,
} from "./defaults";
import { proxyProps } from "./proxyProps";

let locale = "zh-CN";
const storage = {
  getItem: (key: string) => (key === "codex-tools-locale" ? locale : null),
  setItem: () => {},
  removeItem: () => {},
};
Object.assign(globalThis, {
  window: { localStorage: storage, sessionStorage: storage },
});
const render = (node: ReactNode) =>
  renderToStaticMarkup(<I18nProvider>{node}</I18nProvider>);
let count = 0;
const check = (name: string, value: unknown) => {
  assert.ok(value, name);
  count++;
  console.log(`✓ ${name}`);
};
const panel = (html: string, id: string) =>
  html
    .split(
      new RegExp(`<div class="pageSectionPanel" id="[^"]*-${id}-panel"`),
    )[1]
    ?.split('<div class="pageSectionPanel"')[0] ?? "";
const matches = (html: string, pattern: RegExp) => [...html.matchAll(pattern)];
function sectionChecks(html: string, name: string, total: number) {
  check(
    `${name}: only one section is initially selected`,
    matches(html, /aria-selected="true"/g).length === 1,
  );
  check(
    `${name}: all ${total} sections remain mounted`,
    matches(html, /role="tabpanel"/g).length === total,
  );
  check(
    `${name}: other sections are hidden`,
    matches(html, /role="tabpanel"[^>]*hidden=""/g).length === total - 1,
  );
  check(
    `${name}: every tab is connected to its panel`,
    matches(html, /aria-controls="([^"]+)"/g).every((match) =>
      html.includes(`id="${match[1]}" role="tabpanel"`),
    ),
  );
}
const empty = () => {};
const settingsProps: SettingsPanelProps = {
  themeMode: "light",
  onToggleTheme: empty,
  checkingUpdate: false,
  onCheckUpdate: empty,
  onOpenExternalUrl: empty,
  settings: DEFAULT_SETTINGS,
  accounts,
  installedEditorApps: [],
  hasOpencodeDesktopApp: false,
  savingSettings: false,
  onUpdateSettings: empty,
  developerContent: <button>Preview update dialog</button>,
};
const settings = render(<SettingsPanel {...settingsProps} />);
sectionChecks(settings, "Settings", 5);
check(
  "Settings: language and appearance are in General",
  panel(settings, "general").includes("languagePicker") &&
    panel(settings, "general").includes(MESSAGES["zh-CN"].settings.theme.label),
);
check(
  "Settings: startup, account switching and warm-up controls remain",
  panel(settings, "general").includes(
    MESSAGES["zh-CN"].settings.launchAtStartup.label,
  ) &&
    panel(settings, "switching").includes(
      MESSAGES["zh-CN"].settings.launchCodexAfterSwitch.label,
    ) &&
    panel(settings, "warmup").includes("warmupAccountChoices"),
);
check(
  "Settings: project/update links and developer action remain",
  settings.includes("Preview update dialog") &&
    settings.includes("github.com/170-carry/codex-tools"),
);
function QuotaPreview({ platform }: { platform: "macos" | "windows" }) {
  const state = useSettingsWorkspace(settingsProps);
  return (
    <QuotaSettings
      workspace={{
        ...state,
        isMacos: platform === "macos",
        isWindows: platform === "windows",
      }}
    />
  );
}
const macQuota = render(<QuotaPreview platform="macos" />);
const windowsQuota = render(<QuotaPreview platform="windows" />);
check(
  "macOS quota settings: icon styles and percentage variants remain",
  macQuota.includes("trayLogoRingVariant") &&
    matches(macQuota, /aria-pressed=/g).length >= 6,
);
check(
  "Windows quota settings: taskbar and icon choices remain",
  windowsQuota.includes("trayIconStyleOption") &&
    windowsQuota.includes("settingRow"),
);
const analysisProps = {
  analytics,
  error: null,
  loading: false,
  exporting: null,
  progress: null,
  weeklyBudgetUsd: 50,
  savingSettings: false,
  onExport: empty,
  onDeleteSession: empty,
  onUpdateWeeklyBudget: async () => {},
  tokenUsageContent: (
    <TokenUsageStrip
      tokenUsage={tokenUsage}
      error={null}
      text={getUiCopy(locale)}
    />
  ),
};
const analysis = render(<AnalyticsPanel {...analysisProps} />);
sectionChecks(analysis, "Analytics", 4);
check(
  "Analytics: four token periods remain",
  ["24H", "3D", "7D", "30D"].every((value) => analysis.includes(value)),
);
check(
  "Analytics: budgets, projects, sessions, prompts and activity remain",
  [
    "analyticsBudgetControl",
    "analyticsBlockProjects",
    "analyticsBlockSessions",
    "analyticsBlockPrompts",
    "analyticsBlockHeatmap",
  ].every((value) => analysis.includes(value)),
);
check(
  "Analytics: session search has a localized accessible name",
  analysis.includes('aria-label="搜索会话、项目或模型"'),
);
check(
  "Analytics: error and refresh progress stay visible",
  render(
    <AnalyticsPanel {...analysisProps} error="Preview error" loading />,
  ).includes("Preview error"),
);
const emptyAnalysis = render(
  <AnalyticsPanel {...analysisProps} analytics={null} />,
);
check(
  "Analytics: empty data renders without fake totals",
  emptyAnalysis.includes("—") && !emptyAnalysis.includes("NaN"),
);
const proxy = render(<ApiProxyPanel {...proxyProps()} />);
sectionChecks(proxy, "API proxy", 5);
check(
  "Proxy: service controls and port input remain",
  proxy.includes("proxyPortInput") && proxy.includes("8787"),
);
check(
  "Proxy: keys, models, remote and public sections remain",
  [
    "proxyModelCard",
    "proxyKeyManagerCard",
    "remoteSectionHeading",
    "cloudflared",
  ].every((value) => proxy.includes(value)),
);
check(
  "Proxy: no-account start is disabled",
  /<button class="primary"[^>]*disabled=""/.test(panel(proxy, "service")),
);
const configured = proxyProps({
  accountCount: 2,
  loadBalanceMode: "sequential",
  status: {
    ...DEFAULT_API_PROXY_STATUS,
    running: true,
    port: 8787,
    baseUrl: "http://127.0.0.1:8787/v1",
    apiKey: "preview-key",
    codexProxyBound: true,
    codexProxyRestoreAvailable: true,
  },
  apiProxyKeys: [
    {
      id: "preview",
      label: "Example key",
      key: "preview-key",
      enabled: true,
      allowedModels: [],
      allowedReasoningEfforts: [],
      allowedServiceTiers: [],
      createdAt: now,
      updatedAt: now,
    },
  ],
  remoteServers: [
    {
      id: "example",
      label: "Example server",
      host: "example.com",
      sshPort: 22,
      sshUser: "root",
      authMode: "keyPath",
      identityFile: "~/.ssh/example",
      privateKey: null,
      password: null,
      remoteDir: "/opt/codex-tools",
      listenPort: 8787,
    },
  ],
  cloudflaredStatus: {
    ...DEFAULT_CLOUDFLARED_STATUS,
    installed: true,
    running: true,
    tunnelMode: "named",
    customHostname: "proxy.example.com",
  },
});
const fullProxy = render(<ApiProxyPanel {...configured} />);
check(
  "Proxy: sequential limit and model settings remain",
  fullProxy.includes("proxyRangeInput") &&
    fullProxy.includes('aria-valuetext="80%"'),
);
check(
  "Proxy: key permissions and logs are available in a closed disclosure",
  fullProxy.includes('<details class="proxyKeyDetails">') &&
    fullProxy.includes("proxyKeyBindingColumns") &&
    fullProxy.includes("proxyKeyLogs"),
);
check(
  "Proxy: selected remote server and diagnostics remain",
  fullProxy.includes("Example server") && fullProxy.includes("remoteWorkbench"),
);
check(
  "Proxy: named tunnel configuration remains",
  fullProxy.includes("cloudflaredFormGrid") &&
    fullProxy.includes("proxy.example.com"),
);
const chosen = render(
  <PageSections
    label="Example"
    initialId="two"
    sections={[
      { id: "one", label: "One", content: <input defaultValue="draft" /> },
      { id: "two", label: "Two", content: "Two content" },
    ]}
  />,
);
check(
  "Section navigation keeps inactive form drafts in the DOM",
  chosen.includes('value="draft"') &&
    /one-panel[^>]*hidden/.test(chosen) &&
    /two-tab[^>]*aria-selected="true"/.test(chosen),
);
for (const code of ["en-US", "ja-JP", "ko-KR", "ru-RU"]) {
  locale = code;
  check(
    `All secondary pages render in ${code}`,
    [
      render(<SettingsPanel {...settingsProps} />),
      render(<AnalyticsPanel {...analysisProps} />),
      render(<ApiProxyPanel {...configured} />),
    ].every(
      (html) =>
        html.includes('role="tablist"') &&
        !html.includes("undefined") &&
        !html.includes("NaN"),
    ),
  );
}
console.log(
  `\n${count} page render checks passed. Static rendering does not verify browser layout or interactions.`,
);
