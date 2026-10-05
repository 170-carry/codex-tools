import { lazy } from "react";
import type { AppTab } from "../../types/workspace";

const loaders = {
  analytics: () =>
    import("./AnalyticsView").then((module) => ({ default: module.AnalyticsView })),
  proxy: () =>
    import("./ProxyView").then((module) => ({ default: module.ProxyView })),
  settings: () =>
    import("./SettingsView").then((module) => ({ default: module.SettingsView })),
};

// Warm just the view the user is approaching. This also works for keyboard focus
// and leaves startup free of the analytics/proxy/settings chunks.
export function preloadWorkspaceView(tab: AppTab) {
  if (tab !== "accounts") void loaders[tab]().catch(() => {});
}

export const AnalyticsView = lazy(loaders.analytics);
export const ProxyView = lazy(loaders.proxy);
export const SettingsView = lazy(loaders.settings);
