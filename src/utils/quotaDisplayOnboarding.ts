import type {
  AppSettings,
  TrayUsageDisplayMode,
  WindowsTrayIconStyle,
} from "../types/app";

export function hasActiveQuotaDisplay(taskbarEnabled: boolean, trayEnabled: boolean): boolean {
  return taskbarEnabled || trayEnabled;
}

export function canDisableQuotaDisplay(otherDisplayEnabled: boolean): boolean {
  return otherDisplayEnabled;
}

export type QuotaOnboardingPlatform = "windows" | "macos" | null;

export function shouldOpenQuotaOnboarding(options: {
  platform: QuotaOnboardingPlatform;
  settingsLoaded: boolean;
  windowsCompleted: boolean;
  macosCompleted: boolean;
}): boolean {
  if (!options.settingsLoaded) {
    return false;
  }
  if (options.platform === "windows") {
    return !options.windowsCompleted;
  }
  if (options.platform === "macos") {
    return !options.macosCompleted;
  }
  return false;
}

export async function applyLiveQuotaDisplayUpdate<TPatch>(options: {
  patch: TPatch;
  applyLocal: () => void;
  rollbackLocal: () => void;
  persist: (patch: TPatch) => Promise<void>;
}): Promise<boolean> {
  options.applyLocal();
  try {
    await options.persist(options.patch);
    return true;
  } catch {
    options.rollbackLocal();
    return false;
  }
}

export function buildMacosQuotaOnboardingPatch(options: {
  statusBarEnabled: boolean;
  statusBarMode: Exclude<TrayUsageDisplayMode, "hidden">;
  trayEnabled: boolean;
  trayIconStyle: WindowsTrayIconStyle;
  showLogoRingPercentage: boolean;
}): Pick<
  AppSettings,
  | "trayUsageDisplayMode"
  | "windowsTrayIconStyle"
  | "trayQuotaIconVisible"
  | "macosTrayLogoRingShowPercentage"
  | "macosQuotaOnboardingCompleted"
> {
  return {
    trayUsageDisplayMode: options.statusBarEnabled ? options.statusBarMode : "hidden",
    windowsTrayIconStyle: options.trayIconStyle,
    trayQuotaIconVisible: options.trayEnabled,
    macosTrayLogoRingShowPercentage: options.showLogoRingPercentage,
    macosQuotaOnboardingCompleted: true,
  };
}
