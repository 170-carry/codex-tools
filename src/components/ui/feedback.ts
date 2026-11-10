import type { FeedbackKind } from "./FeedbackHost";
export function notifyFeedback(kind: FeedbackKind) { window.dispatchEvent(new CustomEvent("ui-feedback", { detail: kind })); }
