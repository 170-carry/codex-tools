import { SettingsPanel } from "../SettingsPanel";
import type { ThemeMode } from "../../types/app";
import type { CodexController } from "../../types/workspace";
import { useI18n } from "../../i18n/I18nProvider";
import { getWorkspaceCopy } from "../../i18n/workspaceCopy";

export function SettingsView({
  c,
  themeMode,
  toggleTheme,
}: {
  c: CodexController;
  themeMode: ThemeMode;
  toggleTheme: () => void;
}) {
  const { locale } = useI18n();
  const text = getWorkspaceCopy(locale);
  return (
    <SettingsPanel
      developerContent={
        import.meta.env.DEV ? (
          <button type="button" onClick={c.openDebugUpdateDialog}>
            {text.debugUpdate}
          </button>
        ) : undefined
      }
      themeMode={themeMode}
      onToggleTheme={toggleTheme}
      checkingUpdate={c.checkingUpdate}
      onCheckUpdate={() => void c.checkForAppUpdate(false)}
      onOpenExternalUrl={(url) => void c.openExternalUrl(url)}
      settings={c.settings}
      accounts={c.accounts}
      installedEditorApps={c.installedEditorApps}
      hasOpencodeDesktopApp={c.hasOpencodeDesktopApp}
      savingSettings={c.savingSettings}
      onUpdateSettings={(patch, options) =>
        void c.updateSettings(patch, options)
      }
    />
  );
}
