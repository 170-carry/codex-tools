import { Switch } from "./ui/Switch";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { UtilityDialog } from "./workspace/UtilityDialog";
import { useI18n } from "../i18n/I18nProvider";
import {
  activateWindowsTaskbarPlacement,
  applyLiveQuotaDisplayUpdate,
  buildMacosQuotaOnboardingPatch,
  canDisableQuotaDisplay,
  hasActiveQuotaDisplay,
} from "../utils/quotaDisplayOnboarding";
import type {
  AppSettings,
  MacosTrayTextIconStyle,
  TrayUsageDisplayMode,
  WindowsTaskbarWidgetPlacement,
  WindowsTrayIconStyle,
} from "../types/app";
import macosClassicStatusBarExampleUrl from "../assets/macos-text-quota-onboarding-preview.png";

const WINDOWS_TASKBAR_PREVIEW_ASSET_VERSION = "20260811-layout2";

type QuotaDisplayOnboardingDialogProps = {
  open: boolean;
  platform: "windows" | "macos";
  lightTheme: boolean;
  settings: AppSettings;
  saving: boolean;
  onPreviewSettings: (patch: Partial<AppSettings>) => Promise<void>;
  onConfirm: (patch: Partial<AppSettings>) => Promise<void>;
};

type QuotaDisplayOnboardingContentProps = Omit<
  QuotaDisplayOnboardingDialogProps,
  "open" | "platform"
>;

type TrayVisualPreview = {
  style: WindowsTrayIconStyle;
  dataUrl: string;
  pixelWidth: number;
  pixelHeight: number;
};

type OperationError = "preview" | "confirm" | null;

type VisibleTrayUsageDisplayMode = Exclude<TrayUsageDisplayMode, "hidden">;

type MacosTrayIconOption = {
  key: string;
  style: WindowsTrayIconStyle;
  label: string;
  ariaLabel: string;
  showLogoRingPercentage?: boolean;
};

type WindowsTaskbarPreviewProps = {
  showTaskbarQuota?: boolean;
  taskbarPlacement?: Exclude<WindowsTaskbarWidgetPlacement, "hidden">;
  showTrayQuota?: boolean;
  trayPreview?: TrayVisualPreview;
  trayPreviewScale: number;
  windowsWidgetsEnabled?: boolean;
};

function WindowsTaskbarPreview({
  showTaskbarQuota = false,
  taskbarPlacement = "left",
  showTrayQuota = false,
  trayPreview,
  trayPreviewScale,
  windowsWidgetsEnabled = false,
}: WindowsTaskbarPreviewProps) {
  return (
    <div className="quotaOnboardingWindowsPreview" aria-hidden="true">
      <img
        className="quotaPreviewReference"
        src={`/windows-taskbar-preview-no-widgets.png?v=${WINDOWS_TASKBAR_PREVIEW_ASSET_VERSION}`}
        alt=""
        draggable={false}
      />
      {windowsWidgetsEnabled ? (
        <img
          className="quotaPreviewReference quotaPreviewReferenceWidgets"
          src={`/windows-taskbar-preview-left-clean.png?v=${WINDOWS_TASKBAR_PREVIEW_ASSET_VERSION}`}
          alt=""
          draggable={false}
        />
      ) : null}
      {showTrayQuota ? (
        <img
          className="quotaPreviewReference quotaPreviewReferenceTray"
          src={`/windows-taskbar-preview-tray-clean.png?v=${WINDOWS_TASKBAR_PREVIEW_ASSET_VERSION}`}
          alt=""
          draggable={false}
        />
      ) : null}
      {showTaskbarQuota ? (
        <span
          className={`quotaPreviewTaskbarBadge ${
            taskbarPlacement === "left" ? "isLeft" : "isEmbedded"
          } ${windowsWidgetsEnabled ? "hasWindowsWidgets" : ""}`}
        >
          <img src="/codex-tools.png" alt="" draggable={false} />
          <strong>72%</strong>
        </span>
      ) : null}
      {showTrayQuota ? (
        <span className="quotaPreviewTrayIcon isOverlay">
          {trayPreview ? (
            <img
              src={trayPreview.dataUrl}
              alt=""
              draggable={false}
              style={{
                width: `${trayPreview.pixelWidth / trayPreviewScale}px`,
                height: `${trayPreview.pixelHeight / trayPreviewScale}px`,
              }}
            />
          ) : (
            <span className="trayIconPreviewPlaceholder" />
          )}
        </span>
      ) : null}
    </div>
  );
}

export function QuotaDisplayOnboardingDialog({
  open,
  platform,
  lightTheme,
  settings,
  saving,
  onPreviewSettings,
  onConfirm,
}: QuotaDisplayOnboardingDialogProps) {
  if (!open) {
    return null;
  }

  const contentProps = {
    lightTheme,
    settings,
    saving,
    onPreviewSettings,
    onConfirm,
  };
  return platform === "macos" ? (
    <MacosQuotaDisplayOnboardingContent {...contentProps} />
  ) : (
    <QuotaDisplayOnboardingContent {...contentProps} />
  );
}

function MacosQuotaDisplayOnboardingContent({
  lightTheme,
  settings,
  saving,
  onPreviewSettings,
  onConfirm,
}: QuotaDisplayOnboardingContentProps) {
  const { copy } = useI18n();
  const initialStatusBarMode: VisibleTrayUsageDisplayMode =
    settings.trayUsageDisplayMode === "hidden"
      ? "oneWeekRemaining"
      : settings.trayUsageDisplayMode;
  const [statusBarEnabled, setStatusBarEnabled] = useState(
    () => settings.trayUsageDisplayMode !== "hidden",
  );
  const [statusBarMode] = useState<VisibleTrayUsageDisplayMode>(initialStatusBarMode);
  const [trayEnabled, setTrayEnabled] = useState(() => settings.trayQuotaIconVisible);
  const [trayIconStyle, setTrayIconStyle] = useState(() => settings.windowsTrayIconStyle);
  const [showLogoRingPercentage, setShowLogoRingPercentage] = useState(
    () => settings.macosTrayLogoRingShowPercentage,
  );
  const [textQuotaIconStyle, setTextQuotaIconStyle] = useState<MacosTrayTextIconStyle>(
    () => settings.macosTrayTextIconStyle,
  );
  const [trayVisualPreviews, setTrayVisualPreviews] = useState<TrayVisualPreview[]>([]);
  const [applying, setApplying] = useState(false);
  const [operationError, setOperationError] = useState<OperationError>(null);
  const liveUpdateInFlight = useRef(false);
  const trayPreviewScale =
    typeof window !== "undefined" ? Math.max(1, window.devicePixelRatio || 1) : 1;
  const busy = saving || applying;
  const logoProgressRingLabel = copy.settings.windowsTrayIconStyle.logoProgressRing;
  const quotaIconEnabled = trayEnabled;
  const textQuotaDisplayEnabled = statusBarEnabled;
  const logoProgressRingPreview = trayVisualPreviews.find(
    (item) => item.style === "logoProgressRing",
  );
  const trayIconStyleOptions: MacosTrayIconOption[] = [
    {
      key: "gradientNumberPlate",
      style: "gradientNumberPlate",
      label: copy.settings.windowsTrayIconStyle.gradientNumberPlate,
      ariaLabel: copy.settings.windowsTrayIconStyle.gradientNumberPlate,
    },
    {
      key: "gradientNumberCard",
      style: "gradientNumberCard",
      label: copy.settings.windowsTrayIconStyle.gradientNumberCard,
      ariaLabel: copy.settings.windowsTrayIconStyle.gradientNumberCard,
    },
    {
      key: "gradientNumber",
      style: "gradientNumber",
      label: copy.settings.windowsTrayIconStyle.gradientNumber,
      ariaLabel: copy.settings.windowsTrayIconStyle.gradientNumber,
    },
    {
      key: "numberProgressBar",
      style: "numberProgressBar",
      label: copy.settings.windowsTrayIconStyle.numberProgressBar,
      ariaLabel: copy.settings.windowsTrayIconStyle.numberProgressBar,
    },
    {
      key: "logoProgressRing",
      style: "logoProgressRing",
      label: copy.settings.macosTrayLogoRingVariants.withoutPercentage,
      ariaLabel: `${logoProgressRingLabel}: ${copy.settings.macosTrayLogoRingVariants.withoutPercentage}`,
      showLogoRingPercentage: false,
    },
  ];
  useEffect(() => {
    let cancelled = false;
    void invoke<TrayVisualPreview[]>("get_tray_visual_previews", {
      lightTheme,
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
  }, [lightTheme, trayPreviewScale]);


  const runLiveUpdate = async (
    patch: Partial<AppSettings>,
    applyLocal: () => void,
    rollbackLocal: () => void,
  ) => {
    if (busy || liveUpdateInFlight.current) {
      return;
    }
    liveUpdateInFlight.current = true;
    setOperationError(null);
    setApplying(true);
    const applied = await applyLiveQuotaDisplayUpdate({
      patch,
      applyLocal,
      rollbackLocal,
      persist: onPreviewSettings,
    });
    if (!applied) {
      setOperationError("preview");
    }
    liveUpdateInFlight.current = false;
    setApplying(false);
  };

  const toggleTextQuotaDisplay = () => {
    const nextEnabled = !textQuotaDisplayEnabled;
    void runLiveUpdate(
      { trayUsageDisplayMode: nextEnabled ? statusBarMode : "hidden" },
      () => setStatusBarEnabled(nextEnabled),
      () => setStatusBarEnabled(!nextEnabled),
    );
  };

  const selectTextQuotaIconStyle = (nextStyle: MacosTrayTextIconStyle) => {
    if (textQuotaIconStyle === nextStyle && statusBarEnabled) {
      return;
    }
    const previousTextQuotaIconStyle = textQuotaIconStyle;
    const previousStatusBarEnabled = statusBarEnabled;
    void runLiveUpdate(
      {
        macosTrayTextIconStyle: nextStyle,
        trayUsageDisplayMode: statusBarMode,
      },
      () => {
        setTextQuotaIconStyle(nextStyle);
        setStatusBarEnabled(true);
      },
      () => {
        setTextQuotaIconStyle(previousTextQuotaIconStyle);
        setStatusBarEnabled(previousStatusBarEnabled);
      },
    );
  };

  const toggleQuotaIcon = () => {
    const nextEnabled = !quotaIconEnabled;
    void runLiveUpdate(
      { trayQuotaIconVisible: nextEnabled },
      () => setTrayEnabled(nextEnabled),
      () => setTrayEnabled(!nextEnabled),
    );
  };

  const selectTrayIconStyle = (option: MacosTrayIconOption) => {
    const nextShowPercentage = option.showLogoRingPercentage;
    const isSelected =
      option.style === trayIconStyle &&
      trayEnabled &&
      (option.style !== "logoProgressRing" ||
        nextShowPercentage === showLogoRingPercentage);
    if (isSelected) {
      return;
    }
    const previousStyle = trayIconStyle;
    const previousEnabled = trayEnabled;
    const previousShowPercentage = showLogoRingPercentage;
    const patch: Partial<AppSettings> = {
      windowsTrayIconStyle: option.style,
      trayQuotaIconVisible: true,
    };
    if (option.style === "logoProgressRing" && nextShowPercentage !== undefined) {
      patch.macosTrayLogoRingShowPercentage = nextShowPercentage;
    }
    void runLiveUpdate(
      patch,
      () => {
        setTrayIconStyle(option.style);
        setTrayEnabled(true);
        if (option.style === "logoProgressRing" && nextShowPercentage !== undefined) {
          setShowLogoRingPercentage(nextShowPercentage);
        }
      },
      () => {
        setTrayIconStyle(previousStyle);
        setTrayEnabled(previousEnabled);
        setShowLogoRingPercentage(previousShowPercentage);
      },
    );
  };

  const confirm = async () => {
    if (busy) {
      return;
    }
    setOperationError(null);
    try {
      await onConfirm(
        buildMacosQuotaOnboardingPatch({
          statusBarEnabled,
          statusBarMode,
          textIconStyle: textQuotaIconStyle,
          trayEnabled,
          trayIconStyle,
          showLogoRingPercentage,
        }),
      );
    } catch {
      setOperationError("confirm");
    }
  };

  return (
    <UtilityDialog className="quotaOnboardingDialog quotaOnboardingDialogMacos" title={copy.quotaOnboarding.macTitle}
      closeLabel={copy.common.close} showClose={false} dismissible={false} onClose={() => {}}
      actions={<>
          {operationError || applying ? (
            <p
              className={`quotaOnboardingRequirement ${operationError ? "isError" : ""}`}
              role={operationError ? "alert" : "status"}
            >
              {operationError === "preview"
                ? copy.quotaOnboarding.liveUpdateFailed
                : operationError === "confirm"
                  ? copy.quotaOnboarding.saveFailed
                  : copy.quotaOnboarding.macApplying}
            </p>
          ) : null}
          <button
            type="button"
            className="primary quotaOnboardingConfirm"
            disabled={busy}
            onClick={() => void confirm()}
          >
            {saving ? copy.quotaOnboarding.saving : copy.quotaOnboarding.confirm}
          </button>
</>}>
        <div className="quotaOnboardingOptions">
          <section
            className={`quotaOnboardingRow ${textQuotaDisplayEnabled ? "isSelected" : ""}`}
          >
            <header className="quotaOnboardingRowHeader quotaOnboardingInlineRowHeader">
              <span className="quotaOnboardingRowHeaderText">
                <h3>{copy.quotaOnboarding.macStatusBarTitle}</h3>
              </span>
              <Switch label={copy.quotaOnboarding.macStatusBarTitle} checked={textQuotaDisplayEnabled}
                  disabled={busy}
                  onChange={toggleTextQuotaDisplay} />
            </header>

            <div
              className="quotaOnboardingIconGrid quotaOnboardingTextGrid"
              role="radiogroup"
              aria-label={copy.quotaOnboarding.macStatusBarTitle}
            >
              <button
                type="button"
                className={
                  textQuotaDisplayEnabled && textQuotaIconStyle === "codexTools"
                    ? "isSelected"
                    : ""
                }
                aria-pressed={
                  textQuotaDisplayEnabled && textQuotaIconStyle === "codexTools"
                }
                disabled={busy}
                onClick={() => selectTextQuotaIconStyle("codexTools")}
              >
                <span className="quotaOnboardingIconArtwork" aria-hidden="true">
                  <img
                    className="quotaOnboardingClassicTextPreview"
                    src={macosClassicStatusBarExampleUrl}
                    alt=""
                    draggable={false}
                  />
                </span>
                <span>{copy.quotaOnboarding.macCodexToolsIconOption}</span>
              </button>
              <button
                type="button"
                className={
                  textQuotaDisplayEnabled && textQuotaIconStyle === "progressRing"
                    ? "isSelected"
                    : ""
                }
                aria-pressed={
                  textQuotaDisplayEnabled && textQuotaIconStyle === "progressRing"
                }
                disabled={busy}
                onClick={() => selectTextQuotaIconStyle("progressRing")}
              >
                <span className="trayLogoRingVariantArtwork" aria-hidden="true">
                  {logoProgressRingPreview ? (
                    <img
                      src={logoProgressRingPreview.dataUrl}
                      alt=""
                      draggable={false}
                      style={{
                        width: `${logoProgressRingPreview.pixelWidth / trayPreviewScale}px`,
                        height: `${logoProgressRingPreview.pixelHeight / trayPreviewScale}px`,
                      }}
                    />
                  ) : (
                    <span className="trayIconPreviewPlaceholder" />
                  )}
                  <span className="trayLogoRingVariantNumber">97%</span>
                </span>
                <span>{copy.quotaOnboarding.macProgressRingIconOption}</span>
              </button>
            </div>
          </section>

          <section className={`quotaOnboardingRow ${quotaIconEnabled ? "isSelected" : ""}`}>
            <header className="quotaOnboardingRowHeader quotaOnboardingInlineRowHeader">
              <span className="quotaOnboardingRowHeaderText">
                <h3>{copy.quotaOnboarding.macTrayTitle}</h3>
              </span>
              <Switch label={copy.quotaOnboarding.macTrayTitle} checked={quotaIconEnabled}
                  disabled={busy}
                  onChange={toggleQuotaIcon} />
            </header>

            <div
              className="quotaOnboardingIconGrid"
              role="radiogroup"
              aria-label={copy.settings.windowsTrayIconStyle.groupAriaLabel}
            >
              {trayIconStyleOptions.map((option) => {
                const preview = trayVisualPreviews.find((item) => item.style === option.style);
                const selected =
                  quotaIconEnabled &&
                  trayIconStyle === option.style &&
                  (option.style !== "logoProgressRing" ||
                    option.showLogoRingPercentage === showLogoRingPercentage);
                return (
                  <button
                    key={option.key}
                    type="button"
                    className={selected ? "isSelected" : ""}
                    aria-label={option.ariaLabel}
                    aria-pressed={selected}
                    disabled={busy}
                    title={option.ariaLabel}
                    onClick={() => selectTrayIconStyle(option)}
                  >
                    <span className="quotaOnboardingIconArtwork" aria-hidden="true">
                      {preview ? (
                        <img
                          src={preview.dataUrl}
                          alt=""
                          draggable={false}
                          style={{
                            width: `${preview.pixelWidth / trayPreviewScale}px`,
                            height: `${preview.pixelHeight / trayPreviewScale}px`,
                          }}
                        />
                      ) : (
                        <span className="trayIconPreviewPlaceholder" />
                      )}
                    </span>
                    <span>{option.label}</span>
                  </button>
                );
              })}
            </div>
          </section>
        </div>


    </UtilityDialog>
  );
}

function QuotaDisplayOnboardingContent({
  lightTheme,
  settings,
  saving,
  onPreviewSettings,
  onConfirm,
}: QuotaDisplayOnboardingContentProps) {
  const { copy } = useI18n();
  const [taskbarEnabled, setTaskbarEnabled] = useState(
    () => settings.windowsTaskbarWidgetPlacement !== "hidden",
  );
  const [taskbarPlacement, setTaskbarPlacement] = useState<
    Exclude<WindowsTaskbarWidgetPlacement, "hidden">
  >(() => (settings.windowsTaskbarWidgetPlacement === "embedded" ? "embedded" : "left"));
  const [trayEnabled, setTrayEnabled] = useState(() => settings.trayQuotaIconVisible);
  const [trayIconStyle, setTrayIconStyle] = useState(() => settings.windowsTrayIconStyle);
  const [trayVisualPreviews, setTrayVisualPreviews] = useState<TrayVisualPreview[]>([]);
  const [windowsWidgetsEnabled, setWindowsWidgetsEnabled] = useState(false);
  const [windowsWidgetsError, setWindowsWidgetsError] = useState(false);
  const [openingWindowsTaskbarSettings, setOpeningWindowsTaskbarSettings] = useState(false);
  const [applying, setApplying] = useState(false);
  const [operationError, setOperationError] = useState<OperationError>(null);
  const liveUpdateInFlight = useRef(false);
  const trayPreviewScale =
    typeof window !== "undefined" ? Math.max(1, window.devicePixelRatio || 1) : 1;

  const trayIconStyleOptions: Array<{ value: WindowsTrayIconStyle; label: string }> = [
    {
      value: "gradientNumberPlate",
      label: copy.settings.windowsTrayIconStyle.gradientNumberPlate,
    },
    {
      value: "gradientNumberCard",
      label: copy.settings.windowsTrayIconStyle.gradientNumberCard,
    },
    { value: "gradientNumber", label: copy.settings.windowsTrayIconStyle.gradientNumber },
    {
      value: "numberProgressBar",
      label: copy.settings.windowsTrayIconStyle.numberProgressBar,
    },
    { value: "logoProgressRing", label: copy.settings.windowsTrayIconStyle.logoProgressRing },
  ];

  useEffect(() => {
    let cancelled = false;
    void invoke<TrayVisualPreview[]>("get_tray_visual_previews", {
      lightTheme,
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
  }, [lightTheme, trayPreviewScale]);

  useEffect(() => {
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
  }, []);


  const busy = saving || applying;
  const hasActiveDisplay = hasActiveQuotaDisplay(taskbarEnabled, trayEnabled);
  const selectedTrayPreview = trayVisualPreviews.find((item) => item.style === trayIconStyle);

  const runLiveUpdate = async (
    patch: Partial<AppSettings>,
    applyLocal: () => void,
    rollbackLocal: () => void,
  ) => {
    if (busy || liveUpdateInFlight.current) {
      return;
    }
    liveUpdateInFlight.current = true;
    setOperationError(null);
    setApplying(true);
    const applied = await applyLiveQuotaDisplayUpdate({
      patch,
      applyLocal,
      rollbackLocal,
      persist: onPreviewSettings,
    });
    if (!applied) {
      setOperationError("preview");
    }
    liveUpdateInFlight.current = false;
    setApplying(false);
  };

  const toggleTaskbar = () => {
    const nextEnabled = !taskbarEnabled;
    if (!nextEnabled && !canDisableQuotaDisplay(trayEnabled)) {
      return;
    }
    void runLiveUpdate(
      { windowsTaskbarWidgetPlacement: nextEnabled ? taskbarPlacement : "hidden" },
      () => setTaskbarEnabled(nextEnabled),
      () => setTaskbarEnabled(!nextEnabled),
    );
  };

  const toggleTray = () => {
    const nextEnabled = !trayEnabled;
    if (!nextEnabled && !canDisableQuotaDisplay(taskbarEnabled)) {
      return;
    }
    void runLiveUpdate(
      { trayQuotaIconVisible: nextEnabled },
      () => setTrayEnabled(nextEnabled),
      () => setTrayEnabled(!nextEnabled),
    );
  };

  const selectTaskbarPlacement = (
    placement: Exclude<WindowsTaskbarWidgetPlacement, "hidden">,
  ) => {
    if (placement === taskbarPlacement && taskbarEnabled) {
      return;
    }
    const previousPlacement = taskbarPlacement;
    const previousEnabled = taskbarEnabled;
    const selection = activateWindowsTaskbarPlacement(placement);
    void runLiveUpdate(
      selection.patch,
      () => {
        setTaskbarPlacement(selection.taskbarPlacement);
        setTaskbarEnabled(selection.taskbarEnabled);
      },
      () => {
        setTaskbarPlacement(previousPlacement);
        setTaskbarEnabled(previousEnabled);
      },
    );
  };

  const selectTrayIconStyle = (style: WindowsTrayIconStyle) => {
    if (style === trayIconStyle && trayEnabled) {
      return;
    }
    const previousStyle = trayIconStyle;
    const previousEnabled = trayEnabled;
    void runLiveUpdate(
      { windowsTrayIconStyle: style, trayQuotaIconVisible: true },
      () => {
        setTrayIconStyle(style);
        setTrayEnabled(true);
      },
      () => {
        setTrayIconStyle(previousStyle);
        setTrayEnabled(previousEnabled);
      },
    );
  };

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

  const confirm = async () => {
    if (!hasActiveDisplay || busy) {
      return;
    }
    setOperationError(null);
    try {
      await onConfirm({
        trayUsageDisplayMode: "oneWeekRemaining",
        windowsQuotaOnboardingCompleted: true,
      });
    } catch {
      setOperationError("confirm");
    }
  };

  return (
    <UtilityDialog className="quotaOnboardingDialog" title={copy.quotaOnboarding.title} description={copy.quotaOnboarding.description}
      closeLabel={copy.common.close} showClose={false} dismissible={false} onClose={() => {}}
      actions={<>
          {operationError || applying ? (
            <p
              className={`quotaOnboardingRequirement ${operationError ? "isError" : ""}`}
              role={operationError ? "alert" : "status"}
            >
              {operationError === "preview"
                ? copy.quotaOnboarding.liveUpdateFailed
                : operationError === "confirm"
                  ? copy.quotaOnboarding.saveFailed
                  : copy.quotaOnboarding.applying}
            </p>
          ) : null}
          <button
            type="button"
            className="primary quotaOnboardingConfirm"
            disabled={!hasActiveDisplay || busy}
            onClick={() => void confirm()}
          >
            {saving ? copy.quotaOnboarding.saving : copy.quotaOnboarding.confirm}
          </button>
</>}>
        <p className="quotaOnboardingRequirement">{copy.quotaOnboarding.livePreview}</p>
        <div className="quotaOnboardingOptions">
          <section className={`quotaOnboardingRow ${taskbarEnabled ? "isSelected" : ""}`}>
            <header className="quotaOnboardingRowHeader quotaOnboardingInlineRowHeader">
              <h3>{copy.quotaOnboarding.taskbarTitle}</h3>
              <Switch label={copy.quotaOnboarding.taskbarTitle} checked={taskbarEnabled}
                  disabled={busy || (taskbarEnabled && !trayEnabled)}
                  onChange={toggleTaskbar} />
            </header>

            <WindowsTaskbarPreview
              showTaskbarQuota={taskbarEnabled}
              taskbarPlacement={taskbarPlacement}
              trayPreviewScale={trayPreviewScale}
              windowsWidgetsEnabled={windowsWidgetsEnabled}
            />

            <div
              className="modeGroup quotaOnboardingPlacement"
              role="radiogroup"
              aria-label={copy.quotaOnboarding.taskbarPlacementLabel}
            >
              <button
                type="button"
                className={taskbarEnabled && taskbarPlacement === "left" ? "primary" : "ghost"}
                aria-pressed={taskbarEnabled && taskbarPlacement === "left"}
                disabled={busy}
                onClick={() => selectTaskbarPlacement("left")}
              >
                {copy.quotaOnboarding.taskbarLeft}
              </button>
              <button
                type="button"
                className={
                  taskbarEnabled && taskbarPlacement === "embedded" ? "primary" : "ghost"
                }
                aria-pressed={taskbarEnabled && taskbarPlacement === "embedded"}
                disabled={busy}
                onClick={() => selectTaskbarPlacement("embedded")}
              >
                {copy.quotaOnboarding.taskbarRight}
              </button>
            </div>

            {windowsWidgetsEnabled ? (
              <div className="quotaOnboardingWidgetsAction">
                {windowsWidgetsError ? (
                  <span className="settingDescription isError" role="alert">
                    {copy.settings.windowsWidgets.openFailed}
                  </span>
                ) : null}
                <button
                  type="button"
                  className="primary quotaOnboardingWidgetsButton"
                  disabled={busy || openingWindowsTaskbarSettings}
                  onClick={() => void openWindowsTaskbarSettings()}
                  aria-label={copy.settings.windowsWidgets.disableAriaLabel}
                >
                  {copy.settings.windowsWidgets.disable}
                </button>
              </div>
            ) : null}
          </section>

          <section className={`quotaOnboardingRow ${trayEnabled ? "isSelected" : ""}`}>
            <header className="quotaOnboardingRowHeader quotaOnboardingInlineRowHeader">
              <h3>{copy.quotaOnboarding.trayTitle}</h3>
              <Switch label={copy.quotaOnboarding.trayTitle} checked={trayEnabled}
                  disabled={busy || (trayEnabled && !taskbarEnabled)}
                  onChange={toggleTray} />
            </header>

            <WindowsTaskbarPreview
              showTrayQuota={trayEnabled}
              trayPreview={selectedTrayPreview}
              trayPreviewScale={trayPreviewScale}
              windowsWidgetsEnabled={windowsWidgetsEnabled}
            />

            <div
              className="quotaOnboardingIconGrid"
              role="radiogroup"
              aria-label={copy.settings.windowsTrayIconStyle.groupAriaLabel}
            >
              {trayIconStyleOptions.map((option) => {
                const preview = trayVisualPreviews.find((item) => item.style === option.value);
                const selected = trayEnabled && trayIconStyle === option.value;
                return (
                  <button
                    key={option.value}
                    type="button"
                    className={selected ? "isSelected" : ""}
                    aria-label={option.label}
                    aria-pressed={selected}
                    disabled={busy}
                    title={option.label}
                    onClick={() => selectTrayIconStyle(option.value)}
                  >
                    <span className="quotaOnboardingIconArtwork" aria-hidden="true">
                      {preview ? (
                        <img
                          src={preview.dataUrl}
                          alt=""
                          draggable={false}
                          style={{
                            width: `${preview.pixelWidth / trayPreviewScale}px`,
                            height: `${preview.pixelHeight / trayPreviewScale}px`,
                          }}
                        />
                      ) : (
                        <span className="trayIconPreviewPlaceholder" />
                      )}
                    </span>
                    <span>{option.label}</span>
                  </button>
                );
              })}
            </div>
          </section>
        </div>


    </UtilityDialog>
  );
}
