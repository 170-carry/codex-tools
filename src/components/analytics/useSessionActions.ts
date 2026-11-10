import { useState } from "react";
import type { CodexSessionCostBreakdown } from "../../types/app";
import { notifyFeedback } from "../ui/feedback";

export function useSessionActions(
  onDeleteSession: (session: CodexSessionCostBreakdown) => Promise<void> | void,
) {
  const [pendingSession, setPending] = useState<CodexSessionCostBreakdown | null>(null);
  const [deletingSessionId, setDeleting] = useState<string | null>(null);
  const confirmDelete = async () => {
    if (!pendingSession) return;
    setDeleting(pendingSession.sessionId);
    try {
      await onDeleteSession(pendingSession);
      setPending(null);
    } catch {
      notifyFeedback("deleteFailed");
    } finally {
      setDeleting(null);
    }
  };
  return {
    pendingSession,
    pendingDeleteSessionId: pendingSession?.sessionId ?? null,
    deletingSessionId,
    handleDeleteSession: setPending,
    cancelDelete: () => setPending(null),
    confirmDelete,
  };
}
