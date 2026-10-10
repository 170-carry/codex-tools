import { SwitchField } from "../SwitchField";
import type { SettingsWorkspace } from "./useSettingsWorkspace";
export function QuotaSettings({ workspace }: { workspace: SettingsWorkspace }) {
  const {
    settings,
    savingSettings,
    onUpdateSettings,
    copy,
    trayVisualPreviews,
    debugBuild,
    windowsWidgetsEnabled,
    windowsWidgetsError,
    openingWindowsTaskbarSettings,
    isWindows,
    isMacos,
    selectedTrayUsageDisplayMode,
    trayPreviewScale,
    trayIconStyleOptions,
    selectedTrayIconStyle,
    openWindowsTaskbarSettings,
  } = workspace;
  return (
    <div className="settingsGroup">
      {isMacos ? (
        <div>
          <SwitchField
            checked={settings.macosTrayProxyPool}
            onChange={(checked) => onUpdateSettings({ macosTrayProxyPool: checked })}
            label={copy.settings.proxyPoolDisplay.label}
            checkedText=""
            uncheckedText=""
            disabled={savingSettings}
          />
          <p className="settingDescription">{copy.settings.proxyPoolDisplay.description}</p>
        </div>
      ) : null}
      {isMacos || isWindows ? (
        <div className="settingRow settingRowTrayUsage">
          <div className="settingMeta">
            <strong>{copy.settings.trayUsageDisplay.label}</strong>
          </div>
          <div className="trayUsageSettingsControls">
            <div
              className="modeGroup trayUsageModeGroup"
              role="radiogroup"
              aria-label={copy.settings.trayUsageDisplay.groupAriaLabel}
            >
              <button
                className={
                  selectedTrayUsageDisplayMode === "remaining"
                    ? "primary"
                    : "ghost"
                }
                disabled={savingSettings}
                onClick={() =>
                  onUpdateSettings({ trayUsageDisplayMode: "remaining" })
                }
                aria-pressed={selectedTrayUsageDisplayMode === "remaining"}
              >
                {copy.settings.trayUsageDisplay.remaining}
              </button>
              <button
                className={
                  selectedTrayUsageDisplayMode === "used" ? "primary" : "ghost"
                }
                disabled={savingSettings}
                onClick={() =>
                  onUpdateSettings({ trayUsageDisplayMode: "used" })
                }
                aria-pressed={selectedTrayUsageDisplayMode === "used"}
              >
                {copy.settings.trayUsageDisplay.used}
              </button>
              <button
                className={
                  selectedTrayUsageDisplayMode === "fiveHourRemaining"
                    ? "primary"
                    : "ghost"
                }
                disabled={savingSettings}
                onClick={() =>
                  onUpdateSettings({
                    trayUsageDisplayMode: "fiveHourRemaining",
                  })
                }
                aria-pressed={
                  selectedTrayUsageDisplayMode === "fiveHourRemaining"
                }
              >
                {copy.settings.trayUsageDisplay.fiveHourRemaining}
              </button>
              <button
                className={
                  selectedTrayUsageDisplayMode === "oneWeekRemaining"
                    ? "primary"
                    : "ghost"
                }
                disabled={savingSettings}
                onClick={() =>
                  onUpdateSettings({ trayUsageDisplayMode: "oneWeekRemaining" })
                }
                aria-pressed={
                  selectedTrayUsageDisplayMode === "oneWeekRemaining"
                }
              >
                {copy.settings.trayUsageDisplay.oneWeekRemaining}
              </button>
              {isMacos ? (
                <button
                  className={
                    settings.trayUsageDisplayMode === "hidden"
                      ? "primary"
                      : "ghost"
                  }
                  disabled={savingSettings}
                  onClick={() =>
                    onUpdateSettings({ trayUsageDisplayMode: "hidden" })
                  }
                  aria-pressed={settings.trayUsageDisplayMode === "hidden"}
                >
                  {copy.settings.trayUsageDisplay.hidden}
                </button>
              ) : null}
            </div>
            <label
              className="themeSwitch trayUsageTitleSwitch"
              aria-label={copy.settings.trayUsageTitleWindowLabels.label}
              title={
                settings.trayUsageTitleShowWindowLabels
                  ? copy.settings.trayUsageTitleWindowLabels.checkedText
                  : copy.settings.trayUsageTitleWindowLabels.uncheckedText
              }
            >
              <span className="trayUsageTitleSwitchLabel">
                {copy.settings.trayUsageTitleWindowLabels.label}
              </span>
              <input
                type="checkbox"
                checked={settings.trayUsageTitleShowWindowLabels}
                disabled={savingSettings}
                onChange={(event) =>
                  onUpdateSettings({
                    trayUsageTitleShowWindowLabels: event.target.checked,
                  })
                }
              />
              <span className="themeSwitchTrack" aria-hidden="true">
                <span className="themeSwitchThumb" />
              </span>
            </label>
          </div>
        </div>
      ) : null}
      {isWindows || isMacos ? (
        <div className="settingRow settingRowTrayUsage">
          <div className="settingMeta">
            <strong>{copy.settings.windowsTrayIconStyle.label}</strong>
          </div>
          <div className="trayIconStyleControls">
            <div
              className="modeGroup trayUsageModeGroup trayIconStyleGroup"
              role="radiogroup"
              aria-label={copy.settings.windowsTrayIconStyle.groupAriaLabel}
            >
              {trayIconStyleOptions.map((option) => {
                const isHiddenOption = option.value === "hidden";
                const preview = isHiddenOption
                  ? undefined
                  : trayVisualPreviews.find(
                      (item) => item.style === option.value,
                    );
                if (isMacos && option.value === "logoProgressRing") {
                  const styleSelected = selectedTrayIconStyle === option.value;
                  return (
                    <div
                      key={option.value}
                      className={`trayIconStyleOption trayIconStyleCompound ${
                        styleSelected ? "isSelected" : ""
                      }`}
                      role="group"
                      aria-label={option.label}
                    >
                      <span className="trayLogoRingVariantPreviews">
                        {[false, true].map((showPercentage) => {
                          const variantLabel = showPercentage
                            ? copy.settings.macosTrayLogoRingVariants
                                .withPercentage
                            : copy.settings.macosTrayLogoRingVariants
                                .withoutPercentage;
                          const variantSelected =
                            styleSelected &&
                            settings.macosTrayLogoRingShowPercentage ===
                              showPercentage;
                          return (
                            <button
                              key={String(showPercentage)}
                              type="button"
                              className={`trayLogoRingVariant ${
                                variantSelected ? "isSelected" : ""
                              }`}
                              disabled={savingSettings}
                              onClick={() =>
                                onUpdateSettings({
                                  windowsTrayIconStyle: "logoProgressRing",
                                  trayQuotaIconVisible: true,
                                  macosTrayLogoRingShowPercentage:
                                    showPercentage,
                                })
                              }
                              aria-label={`${option.label}：${variantLabel}`}
                              aria-pressed={variantSelected}
                              title={variantLabel}
                            >
                              <span
                                className="trayLogoRingVariantArtwork"
                                aria-hidden="true"
                              >
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
                                {showPercentage ? (
                                  <span className="trayLogoRingVariantNumber">
                                    97%
                                  </span>
                                ) : null}
                              </span>
                            </button>
                          );
                        })}
                      </span>
                      <span className="trayIconStyleLabel">{option.label}</span>
                    </div>
                  );
                }
                return (
                  <button
                    key={option.value}
                    className={`trayIconStyleOption ${
                      selectedTrayIconStyle === option.value
                        ? "primary"
                        : "ghost"
                    }`}
                    disabled={savingSettings}
                    onClick={() => {
                      if (option.value === "hidden") {
                        onUpdateSettings({ trayQuotaIconVisible: false });
                        return;
                      }
                      onUpdateSettings({
                        windowsTrayIconStyle: option.value,
                        trayQuotaIconVisible: true,
                      });
                    }}
                    aria-label={option.label}
                    aria-pressed={selectedTrayIconStyle === option.value}
                    title={option.label}
                  >
                    <span className="trayIconPreviewFrame" aria-hidden="true">
                      {isHiddenOption ? (
                        <span className="trayIconHiddenPreview" />
                      ) : preview ? (
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
                    <span className="trayIconStyleLabel">{option.label}</span>
                  </button>
                );
              })}
            </div>
          </div>
        </div>
      ) : null}
      {isMacos && debugBuild ? (
        <div className="settingRow">
          <div className="settingMeta">
            <strong>{copy.settings.macosQuotaOnboardingPreview.label}</strong>
            <span className="settingDescription">
              {copy.settings.macosQuotaOnboardingPreview.description}
            </span>
          </div>
          <div className="settingActionGroup">
            <button
              type="button"
              className="ghost"
              disabled={savingSettings}
              onClick={() =>
                onUpdateSettings(
                  { macosQuotaOnboardingCompleted: false },
                  { silent: true, throwOnError: true, keepInteractive: true },
                )
              }
            >
              {copy.settings.macosQuotaOnboardingPreview.open}
            </button>
          </div>
        </div>
      ) : null}
      {isWindows ? (
        <div className="settingRow settingRowWindowsTaskbar">
          <div className="settingMeta">
            <strong>{copy.settings.windowsTaskbarWidget.label}</strong>
          </div>
          <div className="windowsTaskbarWidgetControls">
            <div
              className="modeGroup trayUsageModeGroup"
              role="radiogroup"
              aria-label={copy.settings.windowsTaskbarWidget.groupAriaLabel}
            >
              <button
                className={
                  settings.windowsTaskbarWidgetPlacement === "left"
                    ? "primary"
                    : "ghost"
                }
                disabled={savingSettings}
                onClick={() =>
                  onUpdateSettings({ windowsTaskbarWidgetPlacement: "left" })
                }
                aria-pressed={settings.windowsTaskbarWidgetPlacement === "left"}
              >
                {copy.settings.windowsTaskbarWidget.left}
              </button>
              <button
                className={
                  settings.windowsTaskbarWidgetPlacement === "embedded"
                    ? "primary"
                    : "ghost"
                }
                disabled={savingSettings}
                onClick={() =>
                  onUpdateSettings({
                    windowsTaskbarWidgetPlacement: "embedded",
                  })
                }
                aria-pressed={
                  settings.windowsTaskbarWidgetPlacement === "embedded"
                }
              >
                {copy.settings.windowsTaskbarWidget.right}
              </button>
              <button
                className={
                  settings.windowsTaskbarWidgetPlacement === "hidden"
                    ? "primary"
                    : "ghost"
                }
                disabled={savingSettings}
                onClick={() =>
                  onUpdateSettings({ windowsTaskbarWidgetPlacement: "hidden" })
                }
                aria-pressed={
                  settings.windowsTaskbarWidgetPlacement === "hidden"
                }
              >
                {copy.settings.windowsTaskbarWidget.hidden}
              </button>
            </div>
            {windowsWidgetsEnabled ? (
              <div className="windowsWidgetsActionRow">
                {windowsWidgetsError ? (
                  <span className="settingDescription isError" role="alert">
                    {copy.settings.windowsWidgets.openFailed}
                  </span>
                ) : null}
                <button
                  type="button"
                  className="primary windowsWidgetsButton"
                  disabled={openingWindowsTaskbarSettings}
                  onClick={() => void openWindowsTaskbarSettings()}
                  aria-label={copy.settings.windowsWidgets.disableAriaLabel}
                >
                  {copy.settings.windowsWidgets.disable}
                </button>
              </div>
            ) : null}
          </div>
        </div>
      ) : null}
    </div>
  );
}
