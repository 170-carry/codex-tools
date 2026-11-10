import { useEffect, useState, useSyncExternalStore, type ReactNode } from "react";
import { PresenceContext } from "./presenceContext";
const query = "(prefers-reduced-motion: reduce)";
const subscribe = (notify: () => void) => {
  const media = window.matchMedia(query);
  media.addEventListener("change", notify);
  return () => media.removeEventListener("change", notify);
};

/** Retain only the last open view for its short, non-interactive exit. */
export function Presence({ present, children }: { present: boolean; children: ReactNode }) {
  const reduced = useSyncExternalStore(subscribe, () => window.matchMedia(query).matches, () => true);
  const [retained, setRetained] = useState(present);
  const [snapshot, setSnapshot] = useState(children);
  if (present && (!retained || snapshot !== children)) {
    setRetained(true);
    setSnapshot(children);
  }
  useEffect(() => {
    if (present || !retained) return;
    const timer = window.setTimeout(() => setRetained(false), reduced ? 0 : 120);
    return () => window.clearTimeout(timer);
  }, [present, retained, reduced]);
  if (!present && (!retained || reduced)) return null;
  return <PresenceContext value={!present}>{present ? children : snapshot}</PresenceContext>;
}
