import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { createPortal } from "react-dom";
import { useI18n } from "../i18n/I18nProvider";
import {
  applyLiveQuotaDisplayUpdate,
  canDisableQuotaDisplay,
  hasActiveQuotaDisplay,
} from "../utils/quotaDisplayOnboarding";
import type {
  AppSettings,
  WindowsTaskbarWidgetPlacement,
  WindowsTrayIconStyle,
} from "../types/app";

const WINDOWS_TASKBAR_PREVIEW_ASSET_VERSION = "20260811-layout2";

type QuotaDisplayOnboardingDialogProps = {
  open: boolean;
  lightTheme: boolean;
  settings: AppSettings;
  saving: boolean;
  onPreviewSettings: (patch: Partial<AppSettings>) => Promise<void>;
  onConfirm: (patch: Partial<AppSettings>) => Promise<void>;
};

type QuotaDisplayOnboardingContentProps = Omit<
  QuotaDisplayOnboardingDialogProps,
  "open"
>;

type TrayVisualPreview = {
  style: WindowsTrayIconStyle;
  dataUrl: string;
  pixelWidth: number;
  pixelHeight: number;
};

type OperationError = "preview" | "confirm" | null;

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
  lightTheme,
  settings,
  saving,
  onPreviewSettings,
  onConfirm,
}: QuotaDisplayOnboardingDialogProps) {
  if (!open) {
    return null;
  }

  return (
    <QuotaDisplayOnboardingContent
      lightTheme={lightTheme}
      settings={settings}
      saving={saving}
      onPreviewSettings={onPreviewSettings}
      onConfirm={onConfirm}
    />
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
  const dialogRef = useRef<HTMLElement>(null);
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

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) {
      return;
    }

    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const focusableSelector =
      'button:not([disabled]), input:not([disabled]), [href], [tabindex]:not([tabindex="-1"])';
    dialog.querySelector<HTMLElement>(focusableSelector)?.focus();

    const keepFocusInside = (event: KeyboardEvent) => {
      if (event.key !== "Tab") {
        return;
      }
      const focusable = Array.from(dialog.querySelectorAll<HTMLElement>(focusableSelector));
      if (focusable.length === 0) {
        event.preventDefault();
        dialog.focus();
        return;
      }
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };

    document.addEventListener("keydown", keepFocusInside);
    return () => {
      document.removeEventListener("keydown", keepFocusInside);
      previousFocus?.focus();
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
    if (placement === taskbarPlacement || !taskbarEnabled) {
      return;
    }
    const previousPlacement = taskbarPlacement;
    void runLiveUpdate(
      { windowsTaskbarWidgetPlacement: placement },
      () => setTaskbarPlacement(placement),
      () => setTaskbarPlacement(previousPlacement),
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

  return createPortal(
    <div className="quotaOnboardingOverlay">
      <section
        ref={dialogRef}
        className="quotaOnboardingDialog"
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        aria-labelledby="quota-onboarding-title"
        aria-describedby="quota-onboarding-live-preview"
      >
        <header className="quotaOnboardingHeader">
          <h2 id="quota-onboarding-title">{copy.quotaOnboarding.title}</h2>
          <div
            className="quotaOnboardingLiveNotice"
            id="quota-onboarding-live-preview"
            role="status"
          >
            <span aria-hidden="true" />
            {copy.quotaOnboarding.livePreview}
          </div>
        </header>

        <div className="quotaOnboardingOptions">
          <section className={`quotaOnboardingRow ${taskbarEnabled ? "isSelected" : ""}`}>
            <header className="quotaOnboardingRowHeader">
              <h3>{copy.quotaOnboarding.taskbarTitle}</h3>
              <label
                className="quotaOnboardingSwitch"
                title={taskbarEnabled && !trayEnabled ? copy.quotaOnboarding.requireOne : undefined}
              >
                <input
                  type="checkbox"
                  checked={taskbarEnabled}
                  disabled={busy || (taskbarEnabled && !trayEnabled)}
                  onChange={toggleTaskbar}
                />
                <span className="quotaOnboardingSwitchTrack" aria-hidden="true">
                  <span />
                </span>
                <span>{taskbarEnabled ? copy.quotaOnboarding.enabled : copy.quotaOnboarding.enable}</span>
              </label>
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
                className={taskbarPlacement === "left" ? "primary" : "ghost"}
                aria-pressed={taskbarPlacement === "left"}
                disabled={!taskbarEnabled || busy}
                onClick={() => selectTaskbarPlacement("left")}
              >
                {copy.quotaOnboarding.taskbarLeft}
              </button>
              <button
                type="button"
                className={taskbarPlacement === "embedded" ? "primary" : "ghost"}
                aria-pressed={taskbarPlacement === "embedded"}
                disabled={!taskbarEnabled || busy}
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
            <header className="quotaOnboardingRowHeader">
              <h3>{copy.quotaOnboarding.trayTitle}</h3>
              <label
                className="quotaOnboardingSwitch"
                title={trayEnabled && !taskbarEnabled ? copy.quotaOnboarding.requireOne : undefined}
              >
                <input
                  type="checkbox"
                  checked={trayEnabled}
                  disabled={busy || (trayEnabled && !taskbarEnabled)}
                  onChange={toggleTray}
                />
                <span className="quotaOnboardingSwitchTrack" aria-hidden="true">
                  <span />
                </span>
                <span>{trayEnabled ? copy.quotaOnboarding.enabled : copy.quotaOnboarding.enable}</span>
              </label>
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

        <footer className="quotaOnboardingFooter">
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
        </footer>
      </section>
    </div>,
    document.body,
  );
}
