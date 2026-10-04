import {
  useCallback,
  useLayoutEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { AppLayoutContext } from "../../hooks/useAppLayout";
import {
  readAppLayout,
  saveAppLayout,
  type AppLayout,
} from "../../utils/layoutPreference";

function initialPreference(): AppLayout {
  try {
    return readAppLayout(
      typeof window === "undefined" ? undefined : window.localStorage,
    );
  } catch {
    return "classic";
  }
}

export function AppLayoutProvider({
  children,
  initialLayout,
}: {
  children: ReactNode;
  initialLayout?: AppLayout;
}) {
  const [layout, updateLayout] = useState<AppLayout>(
    initialLayout ?? initialPreference,
  );
  const setLayout = useCallback((next: AppLayout) => {
    updateLayout(next);
    try {
      saveAppLayout(window.localStorage, next);
    } catch {
      /* Keep the current window usable when storage is unavailable. */
    }
  }, []);
  useLayoutEffect(() => {
    const root = document.documentElement;
    const body = document.body;
    root.dataset.layout = layout;
    body.classList.toggle("nativeApp", layout === "compact");
    body.classList.toggle("classicApp", layout === "classic");
  }, [layout]);
  const value = useMemo(() => ({ layout, setLayout }), [layout, setLayout]);
  return (
    <AppLayoutContext.Provider value={value}>
      {children}
    </AppLayoutContext.Provider>
  );
}
