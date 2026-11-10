import { Presence } from "./Presence";
import { FadingSurface } from "./FadingSurface";
import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useI18n } from "../../i18n/I18nProvider";
import { uiCopy } from "../../i18n/uiCopy";
export type FeedbackKind = "copied" | "copyFailed" | "saved" | "saveFailed" | "deleteFailed";
export function FeedbackHost() {
  const { locale } = useI18n();
  const [feedback, setFeedback] = useState<{kind: FeedbackKind; id: number} | null>(null);
  useEffect(() => {
    const listener = (event: Event) => setFeedback({ kind: (event as CustomEvent<FeedbackKind>).detail, id: Date.now() });
    window.addEventListener("ui-feedback", listener);
    return () => window.removeEventListener("ui-feedback", listener);
  }, []);
  useEffect(() => {
    if (!feedback) return;
    const timer = window.setTimeout(() => setFeedback(null), feedback.kind.endsWith("Failed") ? 8000 : 4000);
    return () => window.clearTimeout(timer);
  }, [feedback]);
  return createPortal(<Presence present={!!feedback}>{feedback && <FadingSurface className={`uiFeedback ${feedback.kind.endsWith("Failed") ? "isError" : ""}`} role="status" aria-live="polite">
    <span aria-hidden="true">{feedback.kind.endsWith("Failed") ? "!" : "✓"}</span>
    <span>{uiCopy(locale)[feedback.kind]}</span>
    <button type="button" className="uiIconButton" aria-label={uiCopy(locale).close} onClick={() => setFeedback(null)}>×</button>
  </FadingSurface>}</Presence>, document.body);
}
