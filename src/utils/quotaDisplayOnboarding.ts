import type {
  AppSettings,
  MacosTrayTextIconStyle,
  TrayUsageDisplayMode,
  WindowsTaskbarWidgetPlacement,
  WindowsTrayIconStyle,
} from "../types/app";

type EnabledWindowsTaskbarWidgetPlacement = Exclude<
  WindowsTaskbarWidgetPlacement,
  "hidden"
>;

export function effectiveWindowsUsageDisplayMode(
  mode: TrayUsageDisplayMode,
): Exclude<TrayUsageDisplayMode, "hidden"> {
  return mode === "hidden" ? "oneWeekRemaining" : mode;
}

export function activateWindowsTaskbarPlacement(
  placement: EnabledWindowsTaskbarWidgetPlacement,
) {
  return {
    taskbarEnabled: true as const,
    taskbarPlacement: placement,
    patch: { windowsTaskbarWidgetPlacement: placement },
  };
}

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
  textIconStyle: MacosTrayTextIconStyle;
  trayEnabled: boolean;
  trayIconStyle: WindowsTrayIconStyle;
  showLogoRingPercentage: boolean;
}): Pick<
  AppSettings,
  | "trayUsageDisplayMode"
  | "macosTrayTextIconStyle"
  | "windowsTrayIconStyle"
  | "trayQuotaIconVisible"
  | "macosTrayLogoRingShowPercentage"
  | "macosQuotaOnboardingCompleted"
> {
  return {
    trayUsageDisplayMode: options.statusBarEnabled ? options.statusBarMode : "hidden",
    macosTrayTextIconStyle: options.textIconStyle,
    windowsTrayIconStyle: options.trayIconStyle,
    trayQuotaIconVisible: options.trayEnabled,
    macosTrayLogoRingShowPercentage: options.showLogoRingPercentage,
    macosQuotaOnboardingCompleted: true,
  };
}
