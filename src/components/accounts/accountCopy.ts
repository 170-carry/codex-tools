import type { AccountStatus, UiCopy } from "./types";

export function getUiCopy(locale: string): UiCopy {
  if (locale === "zh-CN") {
    return {
      searchPlaceholder: "搜索账号 / 邮箱",
      allStatuses: "全部状态",
      allPlans: "所有套餐",
      proxyEnabled: "反代开启",
      statusUsing: "使用中",
      statusAvailable: "可用",
      statusLow: "即将耗尽",
      statusExhausted: "已耗尽",
      statusIssue: "异常账号",
      issueFallbackReason: "查看账号授权和用量状态",
      tokenUsageTitle: "Token 使用量",
      tokenUsageError: "Token 用量读取失败",
      detailsTitle: "详情",
      usageOverview: "使用概览",
      fiveHourUsage: "5 小时额度",
      weekUsage: "每周额度",
      remainingSuffix: (value) => `剩余 ${value}`,
      resetTime: "重置时间",
      resetCreditsTitle: "重置卡",
      resetCreditsAvailable: (count) =>
        count === null ? "可用数量未知" : `可用 ${count} 张`,
      resetCreditsExpiresAt: "过期时间（系统本地时间）",
      resetCreditsExpand: (hiddenCount) => `展开 ${hiddenCount} 张`,
      resetCreditsCollapse: "收起",
      planType: "套餐类型",
      recentSwitches: "最近切换记录",
      switchRecordAction: "切换到此账号",
      noSwitchRecords: "暂无切换记录",
      fromPrefix: "从",
      quickActions: "快捷操作",
      reauthorize: "重新登录",
      warmup: "激活 5h 窗口",
      warming: "预热中",
      exportAccount: "导出账号",
      exportAll: "全部导出",
      deleteAccount: "删除账号",
      switchAccount: "切换",
      edit: "编辑",
      save: "保存",
      cancel: "取消",
      noMatchesTitle: "没有匹配账号",
      noMatchesDescription: "调整搜索或筛选条件后再查看列表。",
      emptyValue: "--",
    };
  }

  return {
    searchPlaceholder: "Search account / email",
    allStatuses: "All statuses",
    allPlans: "All plans",
    proxyEnabled: "Proxy enabled",
    statusUsing: "In use",
    statusAvailable: "Available",
    statusLow: "Low quota",
    statusExhausted: "Exhausted",
    statusIssue: "Needs attention",
    issueFallbackReason: "Check account auth and usage state",
    tokenUsageTitle: "Token usage",
    tokenUsageError: "Token usage unavailable",
    detailsTitle: "Details",
    usageOverview: "Usage overview",
    fiveHourUsage: "5-hour quota",
    weekUsage: "Weekly quota",
    remainingSuffix: (value) => `${value} remaining`,
    resetTime: "Reset time",
    resetCreditsTitle: "Reset cards",
    resetCreditsAvailable: (count) =>
      count === null ? "Available count unknown" : `${count} available`,
    resetCreditsExpiresAt: "Expires (system local time)",
    resetCreditsExpand: (hiddenCount) => `Show ${hiddenCount} more`,
    resetCreditsCollapse: "Show less",
    planType: "Plan",
    recentSwitches: "Recent switches",
    switchRecordAction: "Switched to this account",
    noSwitchRecords: "No switch records yet",
    fromPrefix: "from",
    quickActions: "Quick actions",
    reauthorize: "Re-login",
    warmup: "Activate 5h window",
    warming: "Warming up",
    exportAccount: "Export account",
    exportAll: "Export all",
    deleteAccount: "Delete",
    switchAccount: "Switch",
    edit: "Edit",
    save: "Save",
    cancel: "Cancel",
    noMatchesTitle: "No matching accounts",
    noMatchesDescription: "Change the search or filters to view accounts.",
    emptyValue: "--",
  };
}

export function statusLabel(status: AccountStatus, text: UiCopy): string {
  if (status === "using") return text.statusUsing;
  if (status === "exhausted") return text.statusExhausted;
  if (status === "low") return text.statusLow;
  if (status === "issue") return text.statusIssue;
  return text.statusAvailable;
}
