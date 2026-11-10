import { useExiting } from "./presenceContext";
import { useEffect, useRef, type ReactNode } from "react";
import { useI18n } from "../../i18n/I18nProvider";
import { uiCopy } from "../../i18n/uiCopy";
import { useInspectorMode } from "../../hooks/useInspectorMode";
import { useModalFocus } from "../../hooks/useModalFocus";

export function Inspector({ title, onClose, children }: { title: string; onClose: () => void; children: ReactNode }) {
  const exiting = useExiting();
  const panel = useRef<HTMLElement>(null);
  const { locale } = useI18n();
  const text = uiCopy(locale);
  const overlay = useInspectorMode(panel);
  useModalFocus(panel, overlay);
  useEffect(() => {
    const trigger = document.activeElement as HTMLElement | null;
    panel.current?.focus({ preventScroll: true });
    return () => { if (trigger?.isConnected) trigger.focus({ preventScroll: true }); };
  }, []);
  return <>
    {overlay && <div className="inspectorBackdrop" data-exiting={exiting || undefined} aria-hidden="true" onClick={onClose} />}
    <section ref={panel} className="accountInspector" data-exiting={exiting || undefined} inert={exiting} tabIndex={-1} role={overlay ? "dialog" : "region"}
      aria-modal={overlay || undefined} aria-label={title}
      onKeyDown={event => { if (event.key === "Escape" && !event.defaultPrevented) { event.stopPropagation(); onClose(); } }}>
      <header className="inspectorToolbar"><strong>{title}</strong>
        <button type="button" className="uiIconButton" onClick={onClose} aria-label={text.close} title={text.close}>
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m6 6 12 12M6 18 18 6" /></svg>
        </button>
      </header>
      <div className="inspectorScroll">{children}</div>
    </section>
  </>;
}
