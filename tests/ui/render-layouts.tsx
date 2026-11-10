import assert from "node:assert/strict";
import fs from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import type { ReactNode } from "react";
import { I18nProvider } from "../../src/i18n/I18nProvider";
import { ClassicAccountDetail } from "../../src/components/classic/ClassicAccountDetail";
import { useClassicAccounts } from "../../src/components/classic/useClassicAccounts";
import { ClassicAccountsGrid } from "../../src/components/classic/ClassicAccountsGrid";
import { ClassicMetaStrip } from "../../src/components/classic/ClassicMetaStrip";
import { ClassicAccountActions } from "../../src/components/classic/ClassicAccountActions";
import { ClaudeAccountDetails } from "../../src/components/claude/ClaudeAccountDetails";
import { ClaudeAccountCard } from "../../src/components/claude/ClaudeAccountCard";
import { ClaudeImportDialog } from "../../src/components/claude/ClaudeImportDialog";
import { ClassicTopBar } from "../../src/components/classic/ClassicTopBar";
import { SettingsPanel } from "../../src/components/SettingsPanel";
import { ApiProxyPanel } from "../../src/components/ApiProxyPanel";
import { AnalyticsPanel } from "../../src/components/AnalyticsPanel";
import { accountProps } from "./accountProps";
import { proxyProps } from "./proxyProps";
import { accounts, analytics, tokenUsage } from "./fixtures";
import { DEFAULT_SETTINGS } from "./defaults";

let locale = "zh-CN";
const storage = {
  getItem: (key: string) => key === "codex-tools-locale" ? locale : null,
  setItem: () => {},
  removeItem: () => {},
};
Object.assign(globalThis, {
  window: { localStorage: storage, sessionStorage: storage },
});
const render = (content: ReactNode) =>
  renderToStaticMarkup(
    <I18nProvider>
      {content}
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
// eslint-disable-next-line react-refresh/only-export-components
function DetailPreview() {
  const workspace = useClassicAccounts({ ...props, tokenUsage, tokenUsageError: null });
  return <ClassicAccountDetail workspace={workspace} />;
}
const detailHtml = render(<DetailPreview />);
check(
  "Original restores four summary cards",
  occurrences(accountHtml, 'class="metaPill metric-') === 4,
);
check(
  "Accounts open with the list and no permanent detail panel",
  accountHtml.includes('class="accountListStack"') &&
    !accountHtml.includes('<aside class="accountDetailPanel"'),
);
check(
  "Original keeps search, filters and adding accounts visible",
  accountHtml.includes("data-account-search") &&
    accountHtml.includes('class="accountFilters"') &&
    accountHtml.includes('class="primary importPrimary"'),
);
check(
  "Original keeps all nine grouped account rows",
  occurrences(accountHtml, '<article class="accountRow ') === 9,
);
check(
  "Original marks exactly one active account",
  occurrences(accountHtml, 'aria-current="true"') === 1,
);
check(
  "Original keeps nine visible proxy switches and reset counts",
  occurrences(accountHtml, 'class="uiSwitch "') === 9 &&
    occurrences(accountHtml, 'class="accountResetCredits') === 9,
);
check(
  "Original exposes details, membership and full reset-credit dates",
  detailHtml.includes('class="detailMetaGrid"') &&
    detailHtml.includes('class="detailCard resetCreditsCard"') &&
    /\d{4}\/\d{2}\/\d{2}/.test(detailHtml),
);
check(
  "Original restores four token periods in the account page",
  ["24H", "3D", "7D", "30D"].every((value) => accountHtml.includes(value)),
);
check(
  "Original has explicit switch, rename and export actions",
  accountHtml.includes('class="rowSwitchButton"') &&
    detailHtml.includes('class="detailEditButton"') &&
    accountHtml.includes("accountTokenExportButton"),
);
check(
  "Account list keeps quotas prominent and moves local usage into the inspector",
  !accountHtml.includes('class="accountStatsCell"') &&
    detailHtml.includes('accountTokenCard') &&
    accountHtml.includes('class="usageMeterReset"'),
);
const legacyAccountHtml = render(
  <ClassicAccountsGrid
    {...props}
    tokenUsage={{ ...tokenUsage, accounts: undefined }}
    tokenUsageError={null}
  />,
);
check(
  "Older snapshots keep account controls while displaying unknown local usage",
  legacyAccountHtml.includes('accountRows') &&
    occurrences(legacyAccountHtml, 'class="uiSwitch "') === 9 &&
    !legacyAccountHtml.includes('NaN'),
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
    !empty.includes("accountDetailPanel") &&
    !empty.includes("NaN"),
);
const classicSettings = render(<SettingsPanel {...settingsProps} />);
check(
  "Settings default to original expanded groups",
  classicSettings.includes("classicSettingsPage") &&
    !classicSettings.includes('role="tabpanel"') &&
    occurrences(classicSettings, 'class="uiSection classicSettingsGroup"') === 5,
);
check(
  "Settings no longer expose layout selection",
  !classicSettings.includes("layoutPicker"),
);
check(
  "Analytics shows hourly overview and organized detail tabs",
  render(<AnalyticsPanel {...analyticsProps} />).includes(
    'class="analyticsOverview"',
  ) &&
    render(<AnalyticsPanel {...analyticsProps} />).includes('role="tabpanel"'),
);
check(
  "Analytics exposes refresh and an export menu",
  ['aria-label="导出"', 'aria-haspopup="menu"', "刷新"].every((value) =>
    render(<AnalyticsPanel {...analyticsProps} />).includes(value),
  ),
);
check(
  "Proxy groups configuration into accessible tabs",
  render(<ApiProxyPanel {...proxyProps()} />).includes("classicProxyPage") &&
    render(<ApiProxyPanel {...proxyProps()} />).includes('role="tabpanel"'),
);
const shared = {
  activeTab: "accounts" as const,
  onSelectTab: action,
  onRefresh: action,
  refreshing: false,
  showRefresh: true,
};
check(
  "The title bar keeps navigation and theme controls without layout selection",
  !render(<ClassicTopBar provider="codex" onProviderChange={action} {...shared} themeMode="light" onToggleTheme={action} onGoHome={action} />).includes("layoutPicker"),
);
const claudeTopbar = render(<ClassicTopBar provider="claude" onProviderChange={action} {...shared} themeMode="light" onToggleTheme={action} onGoHome={action} />);
check("Sidebar identifies the current page for assistive technology", (claudeTopbar.match(/aria-current="page"/g) ?? []).length === 1 && claudeTopbar.includes('aria-label="主导航"'));
check("Toolbar retains theme and refresh actions", claudeTopbar.includes('aria-label="切换主题"') && claudeTopbar.includes('aria-label="手动刷新"'));
check("Provider selector renders the Codex and Claude logos", claudeTopbar.includes('/providers/codex.svg') && claudeTopbar.includes('/providers/claude.svg'));
check("Claude navigation does not expose Codex-only proxy actions", !claudeTopbar.includes('API 反代') && claudeTopbar.includes('aria-label="Claude"'));
const claudeImport = render(<ClaudeImportDialog account={null} onSaved={action} onClose={action} />);
check("Claude authorization accepts its own code and defaults to the browser", claudeImport.includes('code#state') && claudeImport.includes('默认浏览器') && !claudeImport.includes('localhost:1455'));
check("Desktop and CLI logins have distinct import routes", claudeImport.includes('桌面 App 登录') && claudeImport.includes('CLI 登录'));
for (const code of ["en-US", "ja-JP", "ko-KR", "ru-RU"]) {
  locale = code;
  check(
    `Settings render without layout options in ${code}`,
    !render(<SettingsPanel {...settingsProps} />).includes("layoutPicker") &&
      !render(<SettingsPanel {...settingsProps} />).includes("undefined"),
  );
}
check(
  "Choosing a presentation never starts business actions during render",
  actionsCalled === 0,
);
check("Retired layout styles and preference provider are removed",
  !fs.existsSync("src/styles/native") &&
  !fs.existsSync("src/components/layout/AppLayoutProvider.tsx"));
const config = JSON.parse(fs.readFileSync("src-tauri/tauri.conf.json", "utf8"));
check(
  "The initial window restores the original dimensions",
  config.app.windows[0].width === 1586 && config.app.windows[0].height === 992,
);

locale = "zh-CN";
const desktopCard = render(<ClaudeAccountCard a={{id:"desktop", label:"desktop@example.com",email:"desktop@example.com",kind:"desktop",isCurrent:true,expiresAt:Date.now()+3600000,canRefresh:true,plan:"claude_pro",baseUrl:null,model:null,usage:{five_hour:{utilization:31,resets_at:null},seven_day:{utilization:52,resets_at:null}},sync:{lastUsageAt:1730000000,lastRefreshAt:null,needsAuth:false,error:null,retryAt:0}}} busy={false} run={async()=>{}} onReauthorize={action} onDelete={action}/>);
check("Desktop accounts show plan and usage instead of DESKTOP", desktopCard.includes("PRO") && desktopCard.includes("31%") && desktopCard.includes("52%") && !desktopCard.includes("DESKTOP"));
const desktopDetails = render(<ClaudeAccountDetails account={{id:"desktop",label:"Desktop",kind:"desktop",isCurrent:true,canRefresh:true,expiresAt:null,baseUrl:null,model:null,usage:null}} onClose={action}/>);
check("Desktop details expose renewal and the row exposes an action menu", desktopDetails.includes("自动续期已开启") && desktopCard.includes('aria-haspopup="menu"'));

for (const code of ["en-US", "ja-JP", "ko-KR", "ru-RU"]) {
  locale = code;
  const dialog = render(<ClaudeImportDialog account={null} onSaved={action} onClose={action} />);
  check(`Claude import is localized in ${code}`, !dialog.includes("添加 Claude 账号") && !dialog.includes("账号名称") && dialog.includes('aria-modal="true"'));
}
locale = "zh-CN";
const manySessions = Array.from({length: 53}, (_, index) => ({...analytics.sessions[0], sessionId: `test-session-${index}`}));
const paged = render(<AnalyticsPanel {...analyticsProps} analytics={{...analytics, sessions: manySessions}} />);
check("Session history exposes pagination instead of silently truncating at 80", paged.includes("下一页") && paged.includes("53") && !paged.includes('title="test-session-25"') && paged.includes('title="test-session-24"'));

console.log(
  `\n${count} layout render checks passed. No browser interaction or backend actions were performed.`,
);
