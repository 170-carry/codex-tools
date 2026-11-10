import type { AppLocale } from "../i18n/catalog.ts";

export async function persistLocaleWhenReady(state: {
  locale: AppLocale;
  persistedLocale: AppLocale;
  settingsLoaded: boolean;
  recoveryRequired: boolean;
}, persist: (patch: { locale: AppLocale }) => Promise<unknown>): Promise<boolean> {
  if (!state.settingsLoaded || state.recoveryRequired || state.locale === state.persistedLocale) return false;
  await persist({ locale: state.locale });
  return true;
}
