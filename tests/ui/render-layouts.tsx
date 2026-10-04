import assert from "node:assert/strict";
import fs from "node:fs";
import postcss from "postcss";
import { renderToStaticMarkup } from "react-dom/server";
import type { ReactNode } from "react";
import { I18nProvider } from "../../src/i18n/I18nProvider";
import { AppLayoutProvider } from "../../src/components/layout/AppLayoutProvider";
import { LayoutPicker } from "../../src/components/layout/LayoutPicker";
import {
  LAYOUT_STORAGE_KEY,
  type AppLayout,
} from "../../src/utils/layoutPreference";
import { ClassicAccountsGrid } from "../../src/components/classic/ClassicAccountsGrid";
import { ClassicMetaStrip } from "../../src/components/classic/ClassicMetaStrip";
import { ClassicAccountActions } from "../../src/components/classic/ClassicAccountActions";
import { ClassicTopBar } from "../../src/components/classic/ClassicTopBar";
import { AppTopBar } from "../../src/components/AppTopBar";
import { SettingsPanel } from "../../src/components/SettingsPanel";
import { ApiProxyPanel } from "../../src/components/ApiProxyPanel";
import { AnalyticsPanel } from "../../src/components/AnalyticsPanel";
import { accountProps } from "./accountProps";
import { proxyProps } from "./proxyProps";
import { accounts, analytics, tokenUsage } from "./fixtures";
import { DEFAULT_SETTINGS } from "./defaults";

let savedLayout: string | null = null;
let locale = "zh-CN";
const storage = {
  getItem: (key: string) =>
    key === LAYOUT_STORAGE_KEY
      ? savedLayout
      : key === "codex-tools-locale"
        ? locale
        : null,
  setItem: () => {},
  removeItem: () => {},
};
Object.assign(globalThis, {
  window: { localStorage: storage, sessionStorage: storage },
});
const render = (content: ReactNode, layout?: AppLayout) =>
  renderToStaticMarkup(
    <I18nProvider>
      <AppLayoutProvider initialLayout={layout}>{content}</AppLayoutProvider>
    </I18nProvider>,
  );
let count = 0;
const check = (description: string, value: unknown) => {
  assert.ok(value, description);
  count++;
  console.log(`✓ ${description}`);
};
const occurrences = (html: string, value: string) =>
  html.split(value).length - 1;
let actionsCalled = 0;
const action = () => {
  actionsCalled++;
};
const settingsProps = {
  themeMode: "light" as const,
  onToggleTheme: action,
  checkingUpdate: false,
  onCheckUpdate: action,
  onOpenExternalUrl: action,
  settings: DEFAULT_SETTINGS,
  accounts,
  installedEditorApps: [],
  hasOpencodeDesktopApp: false,
  savingSettings: false,
  onUpdateSettings: action,
};
const analyticsProps = {
  analytics,
  error: null,
  loading: false,
  exporting: null,
  progress: null,
  weeklyBudgetUsd: 50,
  savingSettings: false,
  onExport: action,
  onRefresh: action,
  onDeleteSession: action,
  onUpdateWeeklyBudget: async () => {
    action();
  },
};
const original = render(<LayoutPicker />);
check(
  "Default layout picker selects Original",
  original.includes('<option value="classic" selected="">') &&
    !original.includes('<option value="compact" selected="">'),
);
savedLayout = "compact";
check(
  "Saved compact preference is restored",
  render(<LayoutPicker />).includes('<option value="compact" selected="">'),
);
savedLayout = "classic";
check(
  "Saved original preference is restored",
  render(<LayoutPicker />).includes('<option value="classic" selected="">'),
);
savedLayout = "outdated-value";
check(
  "An invalid saved preference renders Original",
  render(<LayoutPicker />).includes('<option value="classic" selected="">'),
);
savedLayout = null;
const props = accountProps({ accounts });
const accountHtml = render(
  <ClassicAccountsGrid
    {...props}
    tokenUsage={tokenUsage}
    tokenUsageError={null}
    leadingContent={
      <ClassicMetaStrip
        accounts={accounts}
        exportingAccounts={false}
        onExportAccounts={action}
      />
    }
    toolbarActions={
      <ClassicAccountActions
        onOpenAddDialog={action}
        onSmartSwitch={action}
        smartSwitching={false}
      />
    }
  />,
);
check(
  "Original restores four summary cards",
  occurrences(accountHtml, 'class="metaPill metric-') === 4,
);
check(
  "Original restores the list and permanent detail panel",
  accountHtml.includes('class="accountListStack"') &&
    accountHtml.includes('<aside class="accountDetailPanel"'),
);
check(
  "Original keeps search, filters and adding accounts visible",
  accountHtml.includes("data-account-search") &&
    accountHtml.includes('class="accountFilters"') &&
    accountHtml.includes('class="primary importPrimary"'),
);
check(
  "Original keeps all eight grouped account rows",
  occurrences(accountHtml, '<article class="accountRow ') === 8,
);
check(
  "Original marks exactly one active account",
  occurrences(accountHtml, 'aria-current="true"') === 1,
);
check(
  "Original keeps eight visible proxy switches and reset counts",
  occurrences(accountHtml, 'class="rowToggle"') === 8 &&
    occurrences(accountHtml, 'class="accountResetCredits') === 8,
);
check(
  "Original exposes details, membership and full reset-credit dates",
  accountHtml.includes('class="detailMetaGrid"') &&
    accountHtml.includes('class="detailCard resetCreditsCard"') &&
    /\d{4}\/\d{2}\/\d{2}/.test(accountHtml),
);
check(
  "Original restores four token periods in the account page",
  ["24H", "3D", "7D", "30D"].every((value) => accountHtml.includes(value)),
);
check(
  "Original has explicit switch, rename and export actions",
  accountHtml.includes('class="rowSwitchButton"') &&
    accountHtml.includes('class="detailEditButton"') &&
    accountHtml.includes("accountTokenExportButton"),
);
const empty = render(
  <ClassicAccountsGrid
    {...accountProps({ accounts: [] })}
    tokenUsage={null}
    tokenUsageError={null}
  />,
);
check(
  "Original handles an empty account list",
  empty.includes("accountEmptyState") &&
    empty.includes("detailEmpty") &&
    !empty.includes("NaN"),
);
const classicSettings = render(<SettingsPanel {...settingsProps} />);
check(
  "Settings default to original expanded groups",
  classicSettings.includes("classicSettingsPage") &&
    !classicSettings.includes('role="tabpanel"') &&
    occurrences(classicSettings, 'class="classicSettingsGroup"') === 5,
);
check(
  "Settings expose the layout picker",
  classicSettings.includes('aria-label="界面布局"'),
);
check(
  "Analytics default to the original overview",
  render(<AnalyticsPanel {...analyticsProps} />).includes(
    'class="analyticsGrid"',
  ) &&
    !render(<AnalyticsPanel {...analyticsProps} />).includes('role="tabpanel"'),
);
check(
  "Original analytics exposes refresh and both export actions",
  ["导出 CSV", "导出 JSON", "刷新"].every((value) =>
    render(<AnalyticsPanel {...analyticsProps} />).includes(value),
  ),
);
check(
  "Proxy defaults to the original stacked sections",
  render(<ApiProxyPanel {...proxyProps()} />).includes("classicProxyPage") &&
    !render(<ApiProxyPanel {...proxyProps()} />).includes('role="tabpanel"'),
);
for (const [name, component, expected] of [
  ["Settings", <SettingsPanel {...settingsProps} />, 5],
  ["Analytics", <AnalyticsPanel {...analyticsProps} />, 4],
  ["Proxy", <ApiProxyPanel {...proxyProps()} />, 5],
] as const) {
  check(
    `${name} still offers compact page sections`,
    occurrences(render(component, "compact"), 'role="tabpanel"') === expected,
  );
}
const shared = {
  activeTab: "accounts" as const,
  onSelectTab: action,
  onRefresh: action,
  refreshing: false,
  showRefresh: true,
};
check(
  "Both title bars offer layout selection",
  render(
    <ClassicTopBar
      {...shared}
      themeMode="light"
      onToggleTheme={action}
      onGoHome={action}
    />,
  ).includes('class="layoutPicker"') &&
    render(
      <AppTopBar
        {...shared}
        searchOpen={false}
        onSearch={action}
        onAddAccount={action}
      />,
      "compact",
    ).includes('class="layoutPicker"'),
);
for (const code of ["en-US", "ja-JP", "ko-KR", "ru-RU"]) {
  locale = code;
  check(
    `Both layout options are available in ${code}`,
    occurrences(render(<LayoutPicker />), "<option value=") === 2 &&
      !render(<LayoutPicker />).includes("undefined"),
  );
}
check(
  "Choosing a presentation never starts business actions during render",
  actionsCalled === 0,
);
for (const file of fs
  .readdirSync("src/styles/native")
  .filter((file) => file.endsWith(".css"))) {
  postcss
    .parse(fs.readFileSync(`src/styles/native/${file}`, "utf8"))
    .walkRules((rule) => {
      if (
        rule.parent?.type === "atrule" &&
        rule.parent.name.endsWith("keyframes")
      )
        return;
      assert.ok(
        rule.selectors.every(
          (selector) =>
            selector.includes('html[data-layout="compact"]') ||
            selector.startsWith(':root[data-layout="compact"]'),
        ),
        `Unscoped compact style in ${file}: ${rule.selector}`,
      );
    });
}
check("Compact CSS cannot override the original palette or layout", true);
const config = JSON.parse(fs.readFileSync("src-tauri/tauri.conf.json", "utf8"));
check(
  "The initial window restores the original dimensions",
  config.app.windows[0].width === 1586 && config.app.windows[0].height === 992,
);
console.log(
  `\n${count} layout render checks passed. No browser interaction or backend actions were performed.`,
);
