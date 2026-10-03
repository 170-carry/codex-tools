import { useId, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useModalFocus } from "../../hooks/useModalFocus";

type UtilityDialogProps = {
  title: string;
  description?: string;
  closeLabel: string;
  onClose: () => void;
  dismissible?: boolean;
  showClose?: boolean;
  role?: "dialog" | "alertdialog";
  className?: string;
  children?: ReactNode;
  actions: ReactNode;
};

// Render this frame only while open so focus is restored to the opening control.
export function UtilityDialog({
  title,
  description,
  closeLabel,
  onClose,
  dismissible = true,
  showClose = true,
  role = "dialog",
  className = "",
  children,
  actions,
}: UtilityDialogProps) {
  const id = useId();
  const root = useRef<HTMLElement>(null);
  useModalFocus(root);
  return createPortal(
    <div
      className="settingsOverlay utilityOverlay"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && dismissible) onClose();
      }}
    >
      <section
        ref={root}
        className={`settingsDialog utilityDialog ${className}`}
        role={role}
        aria-modal="true"
        aria-labelledby={`${id}-title`}
        aria-describedby={description ? `${id}-description` : undefined}
        tabIndex={-1}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            if (dismissible) onClose();
          }
        }}
      >
        <header className="utilityDialogHeader">
          <div>
            <h2 id={`${id}-title`}>{title}</h2>
            {description ? <p id={`${id}-description`}>{description}</p> : null}
          </div>
          {showClose ? (
            <button
              type="button"
              className="iconButton ghost closeButton"
              aria-label={closeLabel}
              title={closeLabel}
              disabled={!dismissible}
              onClick={onClose}
            >
              <svg className="iconGlyph" viewBox="0 0 24 24" aria-hidden="true">
                <path d="m6 6 12 12M18 6 6 18" />
              </svg>
            </button>
          ) : null}
        </header>
        {children ? (
          <div className="utilityDialogContent">{children}</div>
        ) : null}
        <footer className="utilityDialogActions">{actions}</footer>
      </section>
    </div>,
    document.body,
  );
}
