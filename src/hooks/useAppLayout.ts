import { createContext, useContext } from "react";
import { DEFAULT_APP_LAYOUT, type AppLayout } from "../utils/layoutPreference";

export const AppLayoutContext = createContext<{
  layout: AppLayout;
  setLayout: (layout: AppLayout) => void;
}>({
  layout: DEFAULT_APP_LAYOUT,
  setLayout: () => {},
});
export function useAppLayout() {
  return useContext(AppLayoutContext);
}
