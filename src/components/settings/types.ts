import type { ReactNode } from "react";
import type {
  AppSettings,
  AccountSummary,
  InstalledEditorApp,
  ThemeMode,
  UpdateSettingsOptions,
  WindowsTrayIconStyle,
} from "../../types/app";
export type SettingsPanelProps = {
  developerContent?: ReactNode;
  themeMode: ThemeMode;
  onToggleTheme: () => void;
  checkingUpdate: boolean;
  onCheckUpdate: () => void;
  onOpenExternalUrl: (url: string) => void;
  settings: AppSettings;
  accounts: AccountSummary[];
  installedEditorApps: InstalledEditorApp[];
  hasOpencodeDesktopApp: boolean;
  savingSettings: boolean;
  onUpdateSettings: (
    patch: Partial<AppSettings>,
    options?: UpdateSettingsOptions,
  ) => void;
};
export type TrayVisualPreview = {
  style: WindowsTrayIconStyle;
  dataUrl: string;
  pixelWidth: number;
  pixelHeight: number;
};
