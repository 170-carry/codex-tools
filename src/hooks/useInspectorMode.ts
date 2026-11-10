import { useLayoutEffect, useState, type RefObject } from "react";

// Mirrors the accounts container breakpoint, never the window width.
export function useInspectorMode(panel: RefObject<HTMLElement | null>) {
  const [overlay, setOverlay] = useState(false);
  useLayoutEffect(() => {
    const workspace = panel.current?.closest<HTMLElement>(".accountsLayout");
    if (!workspace) return;
    const observer = new ResizeObserver(([entry]) => setOverlay(entry.contentRect.width < 1120));
    observer.observe(workspace);
    return () => observer.disconnect();
  }, [panel]);
  return overlay;
}
