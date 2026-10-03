import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../../i18n/I18nProvider";
import { effectiveWindowsUsageDisplayMode } from "../../utils/quotaDisplayOnboarding";
import type { WindowsTrayIconStyle } from "../../types/app";
import type { SettingsPanelProps, TrayVisualPreview } from "./types";
export function useSettingsWorkspace({
  developerContent,
  themeMode,
  onToggleTheme,
  checkingUpdate,
  onCheckUpdate,
  onOpenExternalUrl,
  settings,
  accounts,
  installedEditorApps,
  hasOpencodeDesktopApp,
  savingSettings,
  onUpdateSettings,
}: SettingsPanelProps) {
  const { copy, locale, localeOptions, setLocale } = useI18n();
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const [trayVisualPreviews, setTrayVisualPreviews] = useState<
    TrayVisualPreview[]
  >([]);
  const [runtimePlatform, setRuntimePlatform] = useState<string | null>(null);
  const [debugBuild, setDebugBuild] = useState(false);
  const [pickingCodexLaunchPathKind, setPickingCodexLaunchPathKind] = useState<
    "file" | "directory" | null
  >(null);
  const [windowsWidgetsEnabled, setWindowsWidgetsEnabled] = useState(false);
  const [windowsWidgetsError, setWindowsWidgetsError] = useState(false);
  const [openingWindowsTaskbarSettings, setOpeningWindowsTaskbarSettings] =
    useState(false);
  const languageLabel = copy.topBar.languagePicker;
  const languageOptions = localeOptions.map((item) => ({
    id: item.code,
    label: item.nativeLabel,
  }));
  const versionValue = appVersion ? `v${appVersion}` : "...";
  const isWindows = runtimePlatform === "windows";
  const isMacos = runtimePlatform === "macos";
  const selectedTrayUsageDisplayMode = isWindows
    ? effectiveWindowsUsageDisplayMode(settings.trayUsageDisplayMode)
    : settings.trayUsageDisplayMode;
  const trayPreviewScale =
    typeof window !== "undefined"
      ? Math.max(1, window.devicePixelRatio || 1)
      : 1;
  const trayIconStyleOptions: Array<{
    value: WindowsTrayIconStyle | "hidden";
    label: string;
  }> = [
    {
      value: "gradientNumberPlate",
      label: copy.settings.windowsTrayIconStyle.gradientNumberPlate,
    },
    {
      value: "gradientNumberCard",
      label: copy.settings.windowsTrayIconStyle.gradientNumberCard,
    },
    {
      value: "gradientNumber",
      label: copy.settings.windowsTrayIconStyle.gradientNumber,
    },
    {
      value: "numberProgressBar",
      label: copy.settings.windowsTrayIconStyle.numberProgressBar,
    },
    {
      value: "logoProgressRing",
      label: copy.settings.windowsTrayIconStyle.logoProgressRing,
    },
  ];
  trayIconStyleOptions.push({
    value: "hidden",
    label: copy.settings.windowsTrayIconStyle.hidden,
  });
  const selectedTrayIconStyle = !settings.trayQuotaIconVisible
    ? "hidden"
    : settings.windowsTrayIconStyle;
  const warmupAccounts = Array.from(
    new Map(
      accounts
        .filter((account) => account.sourceKind !== "relay")
        .map((account) => [account.accountKey, account]),
    ).values(),
  );

  const toggleWarmupAccount = (accountId: string, enabled: boolean) => {
    const selected = new Set(settings.autoAccountWarmupAccountIds);
    if (enabled) {
      selected.add(accountId);
    } else {
      selected.delete(accountId);
    }
    onUpdateSettings({ autoAccountWarmupAccountIds: Array.from(selected) });
  };

  useEffect(() => {
    let cancelled = false;

    void getVersion()
      .then((version) => {
        if (!cancelled) {
          setAppVersion(version);
        }
      })
      .catch(() => {});

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    void invoke<string>("get_runtime_platform")
      .then((platform) => {
        if (!cancelled) {
          setRuntimePlatform(platform);
        }
      })
      .catch(() => {
        if (!cancelled) {
          // 浏览器预览没有 Tauri 命令；仅为本地预览保留平台回退，桌面包始终以后端为准。
          const platform = navigator.platform.toLowerCase();
          setRuntimePlatform(
            platform.includes("mac")
              ? "macos"
              : platform.includes("win")
                ? "windows"
                : "other",
          );
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    void invoke<boolean>("is_debug_build")
      .then((enabled) => {
        if (!cancelled) {
          setDebugBuild(enabled);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setDebugBuild(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!isWindows && !isMacos) {
      return;
    }

    let cancelled = false;
    void invoke<TrayVisualPreview[]>("get_tray_visual_previews", {
      lightTheme: themeMode !== "dark",
      devicePixelRatio: trayPreviewScale,
    })
      .then((previews) => {
        if (!cancelled) {
          setTrayVisualPreviews(previews);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setTrayVisualPreviews([]);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [isMacos, isWindows, themeMode, trayPreviewScale]);

  useEffect(() => {
    if (!isWindows) {
      return;
    }

    let cancelled = false;
    const refreshWindowsWidgetsState = () => {
      void invoke<boolean>("get_windows_widgets_enabled")
        .then((enabled) => {
          if (!cancelled) {
            setWindowsWidgetsEnabled(enabled);
          }
        })
        .catch(() => {
          if (!cancelled) {
            setWindowsWidgetsEnabled(false);
          }
        });
    };
    const handleVisibilityChange = () => {
      if (document.visibilityState === "visible") {
        refreshWindowsWidgetsState();
      }
    };

    refreshWindowsWidgetsState();
    window.addEventListener("focus", refreshWindowsWidgetsState);
    document.addEventListener("visibilitychange", handleVisibilityChange);

    return () => {
      cancelled = true;
      window.removeEventListener("focus", refreshWindowsWidgetsState);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, [isWindows]);

  const openWindowsTaskbarSettings = async () => {
    if (openingWindowsTaskbarSettings) {
      return;
    }
    setOpeningWindowsTaskbarSettings(true);
    setWindowsWidgetsError(false);
    try {
      await invoke("open_windows_taskbar_settings");
    } catch {
      setWindowsWidgetsError(true);
    } finally {
      setOpeningWindowsTaskbarSettings(false);
    }
  };

  const pickCodexLaunchPath = async (kind: "file" | "directory") => {
    if (savingSettings || pickingCodexLaunchPathKind) {
      return;
    }

    setPickingCodexLaunchPathKind(kind);
    try {
      const selected = await invoke<string | null>("pick_codex_launch_path", {
        kind,
        currentPath: settings.codexLaunchPath,
      });
      if (!selected) {
        return;
      }
      onUpdateSettings({ codexLaunchPath: selected });
    } finally {
      setPickingCodexLaunchPathKind(null);
    }
  };

  return {
    developerContent,
    themeMode,
    onToggleTheme,
    checkingUpdate,
    onCheckUpdate,
    onOpenExternalUrl,
    settings,
    accounts,
    installedEditorApps,
    hasOpencodeDesktopApp,
    savingSettings,
    onUpdateSettings,
    copy,
    locale,
    localeOptions,
    setLocale,
    appVersion,
    setAppVersion,
    trayVisualPreviews,
    setTrayVisualPreviews,
    runtimePlatform,
    setRuntimePlatform,
    debugBuild,
    setDebugBuild,
    pickingCodexLaunchPathKind,
    setPickingCodexLaunchPathKind,
    windowsWidgetsEnabled,
    setWindowsWidgetsEnabled,
    windowsWidgetsError,
    setWindowsWidgetsError,
    openingWindowsTaskbarSettings,
    setOpeningWindowsTaskbarSettings,
    languageLabel,
    languageOptions,
    versionValue,
    isWindows,
    isMacos,
    selectedTrayUsageDisplayMode,
    trayPreviewScale,
    trayIconStyleOptions,
    selectedTrayIconStyle,
    warmupAccounts,
    toggleWarmupAccount,
    openWindowsTaskbarSettings,
    pickCodexLaunchPath,
  };
}

export type SettingsWorkspace = ReturnType<typeof useSettingsWorkspace>;
