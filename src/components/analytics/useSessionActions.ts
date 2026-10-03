import { useEffect, useRef, useState } from "react";
import type { CodexSessionCostBreakdown } from "../../types/app";

export function useSessionActions(
  onDeleteSession: (session: CodexSessionCostBreakdown) => Promise<void> | void,
) {
  const [pendingDeleteSessionId, setPending] = useState<string | null>(null);
  const [deletingSessionId, setDeleting] = useState<string | null>(null);
  const timer = useRef<number | null>(null);
  useEffect(
    () => () => {
      if (timer.current !== null) window.clearTimeout(timer.current);
    },
    [],
  );
  const handleDeleteSession = (session: CodexSessionCostBreakdown) => {
    if (deletingSessionId !== null) return;
    if (timer.current !== null) window.clearTimeout(timer.current);
    if (pendingDeleteSessionId !== session.sessionId) {
      setPending(session.sessionId);
      timer.current = window.setTimeout(() => {
        setPending((current) =>
          current === session.sessionId ? null : current,
        );
        timer.current = null;
      }, 3000);
      return;
    }
    timer.current = null;
    setDeleting(session.sessionId);
    void Promise.resolve(onDeleteSession(session))
      .catch(() => {})
      .finally(() => {
        setPending(null);
        setDeleting(null);
      });
  };
  return { pendingDeleteSessionId, deletingSessionId, handleDeleteSession };
}
