/* eslint-disable react-refresh/only-export-components */
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import {
  DEFAULT_LOCALE,
  getNextLocale,
  isSupportedLocale,
  LOCALE_OPTIONS,
  MESSAGES,
  type AppLocale,
  type LocaleOption,
  type MessageCatalog,
} from "./catalog";

const LOCALE_STORAGE_KEY = "codex-tools-locale";

type I18nContextValue = {
  locale: AppLocale;
  localeOptions: LocaleOption[];
  copy: MessageCatalog;
  setLocale: (next: AppLocale) => void;
  toggleLocale: () => void;
};

function detectBrowserLocale(): AppLocale {
  if (typeof navigator === "undefined") {
    return DEFAULT_LOCALE;
  }

  const candidates = [navigator.language, ...(navigator.languages ?? [])]
    .filter(Boolean)
    .map((value) => value.toLowerCase());

  for (const value of candidates) {
    if (value.startsWith("zh")) {
      return "zh-CN";
    }
    if (value.startsWith("en")) {
      return "en-US";
    }
    if (value.startsWith("ja")) {
      return "ja-JP";
    }
    if (value.startsWith("ko")) {
      return "ko-KR";
    }
    if (value.startsWith("ru")) {
      return "ru-RU";
    }
  }

  return DEFAULT_LOCALE;
}

function readInitialLocale(): AppLocale {
  if (typeof window === "undefined") {
    return DEFAULT_LOCALE;
  }

  const stored = window.localStorage.getItem(LOCALE_STORAGE_KEY);
  if (isSupportedLocale(stored)) {
    return stored;
  }

  return detectBrowserLocale();
}

const I18nContext = createContext<I18nContextValue | null>(null);

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<AppLocale>(() => readInitialLocale());

  const setLocale = useCallback((next: AppLocale) => {
    setLocaleState(next);
  }, []);

  const toggleLocale = useCallback(() => {
    setLocaleState((current) => getNextLocale(current));
  }, []);

  useEffect(() => {
    if (typeof document !== "undefined") {
      document.documentElement.lang = locale;
    }
    if (typeof window !== "undefined") {
      window.localStorage.setItem(LOCALE_STORAGE_KEY, locale);
    }
  }, [locale]);

  const value = useMemo<I18nContextValue>(() => {
    return {
      locale,
      localeOptions: LOCALE_OPTIONS,
      copy: MESSAGES[locale],
      setLocale,
      toggleLocale,
    };
  }, [locale, setLocale, toggleLocale]);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n() {
  const context = useContext(I18nContext);
  if (!context) {
    throw new Error("useI18n must be used within I18nProvider");
  }
  return context;
}
