/* eslint-disable react-refresh/only-export-components -- Node SSR test entry; no Fast Refresh runtime. */
import assert from "node:assert/strict";
import { renderToStaticMarkup } from "react-dom/server";
import type { ReactNode } from "react";
import { MESSAGES } from "../../src/i18n/catalog";
import { I18nProvider } from "../../src/i18n/I18nProvider";
import { AnalyticsPanel } from "../../src/components/AnalyticsPanel";
import { ApiProxyPanel } from "../../src/components/ApiProxyPanel";
import { SettingsPanel } from "../../src/components/SettingsPanel";
import { QuotaSettings } from "../../src/components/settings/QuotaSettings";
import { useSettingsWorkspace } from "../../src/components/settings/useSettingsWorkspace";
import type { SettingsPanelProps } from "../../src/components/settings/types";
import { accounts, analytics, tokenUsage, now } from "./fixtures";
import {
  DEFAULT_SETTINGS,
  DEFAULT_API_PROXY_STATUS,
  DEFAULT_CLOUDFLARED_STATUS,
} from "./defaults";
import { proxyProps } from "./proxyProps";
import { accountProps } from "./accountProps";
import { fixtureAccount } from "./fixtures";
import { ClassicAccountsGrid } from "../../src/components/classic/ClassicAccountsGrid";
import type { AccountsGridProps } from "../../src/components/accounts/types";
function AccountsGrid(props: AccountsGridProps) {
  return <ClassicAccountsGrid {...props} tokenUsage={tokenUsage} tokenUsageError={null} />;
}

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
  renderToStaticMarkup(
    <I18nProvider>
      {node}
    </I18nProvider>,
  );
let count = 0;
const check = (name: string, value: unknown) => {
  assert.ok(value, name);
  count++;
  console.log(`✓ ${name}`);
};
const matches = (html: string, pattern: RegExp) => [...html.matchAll(pattern)];
function sectionChecks(html: string, name: string) {
  check(`${name}: all content is available in the main page`,
    !html.includes('role="tabpanel"') && !html.includes('pageSectionNav'));
}
const empty = () => {};
const imported = [
  fixtureAccount("second", "second@example.com", 1, 1, {
    addedAt: 20,
    isCurrent: true,
  }),
  fixtureAccount("first", "first@example.com", 99, 99, { addedAt: 10 }),
];
const ordered = render(
  <AccountsGrid {...accountProps({ accounts: imported })} />,
);
check(
  "Accounts: current status and quota ranking preserve import order",
  ordered.indexOf("first@example.com") < ordered.indexOf("second@example.com"),
);
const refreshed = render(
  <AccountsGrid
    {...accountProps({
      accounts: imported.map((account) => ({
        ...account,
        isCurrent: account.id === "first",
        label: `${account.id}@example.com`,
        usage: {
          ...account.usage!,
          fiveHour: {
            ...account.usage!.fiveHour!,
            usedPercent: 100 - account.usage!.fiveHour!.usedPercent!,
          },
        },
      })),
    })}
  />,
);
check(
  "Accounts: refresh and switching keep the same row order",
  refreshed.indexOf("first@example.com") <
    refreshed.indexOf("second@example.com"),
);
const proxyRows = render(
  <AccountsGrid
    {...accountProps({
      accounts: [
        fixtureAccount("current", "current@example.com", 10, 10, {
          isCurrent: true,
          apiProxyEnabled: true,
        }),
        fixtureAccount("disabled", "disabled@example.com", 20, 20, {
          apiProxyEnabled: false,
        }),
        fixtureAccount("relay", "Relay", 0, 0, {
          sourceKind: "relay",
          apiProxyEnabled: true,
        }),
      ],
    })}
  />,
);
const proxyInputs = matches(proxyRows, /<label class="rowToggle"[^>]*><input[^>]*>/g).map(
  (match) => match[0],
);
check(
  "Accounts: every row, including current and Relay, exposes a proxy switch",
  proxyInputs.length === 3,
);
check(
  "Accounts: proxy switches reflect saved enabled and disabled states",
  proxyInputs.filter((input) => input.includes('checked=""')).length === 2,
);
check(
  "Accounts: proxy switches identify their account and purpose",
  proxyInputs.every(
    (input) =>
      input.includes('aria-label="') &&
      input.includes(MESSAGES["zh-CN"].accountCard.apiProxyToggle),
  ),
);
const busyRows = render(
  <AccountsGrid {...accountProps({ authBusy: true, accounts: imported })} />,
);
check(
  "Accounts: auth operations disable the visible proxy switches",
  matches(busyRows, /<label class="rowToggle"[^>]*><input[^>]*>/g).every((match) =>
    match[0].includes('disabled=""'),
  ),
);
check(
  "Accounts: reset credits no longer have a separate column",
  !proxyRows.includes("accountCreditsColumn") &&
    !proxyRows.includes("accountCreditsCell") &&
    !proxyRows.includes("accountTable"),
);
check(
  "Accounts: reset credit badges follow the plan tags",
  matches(
    proxyRows,
    /class="accountStateLine".*?<span class="accountResetCredits/g,
  ).length === 3,
);
const unknownAndZero = render(
  <AccountsGrid
    {...accountProps({
      accounts: [
        fixtureAccount("zero", "Zero", 0, 0, {
          usage: {
            ...imported[0].usage!,
            resetCredits: { availableCount: 0, credits: [] },
          },
        }),
        fixtureAccount("unknown", "Unknown", 0, 0, {
          usage: {
            ...imported[0].usage!,
            resetCredits: { availableCount: null, credits: [] },
          },
        }),
      ],
    })}
  />,
);
check(
  "Accounts: inline reset credits keep zero and unknown counts distinct",
  unknownAndZero.includes("accountResetCredits isEmpty") &&
    unknownAndZero.includes("<strong>0</strong>") &&
    unknownAndZero.includes("<strong>—</strong>"),
);
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
sectionChecks(settings, "Settings");
check(
  "Settings: language and appearance are in General",
  settings.includes("languagePicker") &&
    settings.includes(MESSAGES["zh-CN"].settings.theme.label),
);
check(
  "Settings: startup, account switching and warm-up controls remain",
  settings.includes(
    MESSAGES["zh-CN"].settings.launchAtStartup.label,
  ) &&
    settings.includes(
      MESSAGES["zh-CN"].settings.launchCodexAfterSwitch.label,
    ) &&
    settings.includes("warmupAccountChoices"),
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

};
const analysis = render(<AnalyticsPanel {...analysisProps} />);
sectionChecks(analysis, "Analytics");
check(
  "Analytics: four summary metrics remain",
  matches(analysis, /class="analyticsStatCard"/g).length === 4,
);
check(
  "Analytics: budgets, projects, sessions, prompts and activity remain",
  [
    "analyticsBudgetActions",
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
sectionChecks(proxy, "API proxy");
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
  /<button class="primary"[^>]*disabled=""/.test(proxy),
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
  "Proxy: key permissions and logs are available in an expanded disclosure",
  fullProxy.includes('<details class="proxyKeyDetails" open="">') &&
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
        !html.includes('role="tabpanel"') &&
        !html.includes("undefined") &&
        !html.includes("NaN"),
    ),
  );
}
console.log(
  `\n${count} page render checks passed. Static rendering does not verify browser layout or interactions.`,
);
