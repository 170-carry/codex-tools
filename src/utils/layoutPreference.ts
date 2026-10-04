export type AppLayout = "classic" | "compact";
export const LAYOUT_STORAGE_KEY = "codex-tools-layout";
export const DEFAULT_APP_LAYOUT: AppLayout = "classic";

type LayoutStorage = {
  getItem: (key: string) => string | null;
  setItem: (key: string, value: string) => void;
};

export function parseAppLayout(value: unknown): AppLayout {
  return value === "compact" ? "compact" : DEFAULT_APP_LAYOUT;
}

export function readAppLayout(
  storage?: Pick<LayoutStorage, "getItem">,
): AppLayout {
  try {
    return parseAppLayout(storage?.getItem(LAYOUT_STORAGE_KEY));
  } catch {
    return DEFAULT_APP_LAYOUT;
  }
}

export function saveAppLayout(
  storage: Pick<LayoutStorage, "setItem"> | undefined,
  layout: AppLayout,
): boolean {
  try {
    if (!storage) return false;
    storage.setItem(LAYOUT_STORAGE_KEY, layout);
    return true;
  } catch {
    return false;
  }
}
