import { useId } from "react";
import { createPortal } from "react-dom";
import { useAnchoredMenu } from "../../hooks/useAnchoredMenu";
import { WorkspaceIcon, type WorkspaceIconName } from "./WorkspaceIcon";

export function ActionMenu({
  label,
  actions,
  disabled = false,
}: {
  label: string;
  disabled?: boolean;
  actions: {
    label: string;
    icon?: WorkspaceIconName;
    disabled?: boolean;
    onClick: () => void;
  }[];
}) {
  const {
    open,
    toggle,
    close,
    triggerRef,
    menuRef,
    position,
    portalContainer,
  } = useAnchoredMenu();
  const id = useId();
  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        className="pageActionMenuButton"
        disabled={disabled}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        onClick={toggle}
      >
        <span>{label}</span>
        <WorkspaceIcon name="chevron" />
      </button>
      {open && portalContainer
        ? createPortal(
            <div
              ref={menuRef}
              id={id}
              className="accountActionMenu"
              role="menu"
              aria-label={label}
              style={position}
            >
              {actions.map((action) => (
                <button
                  key={action.label}
                  type="button"
                  role="menuitem"
                  disabled={action.disabled}
                  onClick={() => {
                    close(true);
                    action.onClick();
                  }}
                >
                  {action.icon ? <WorkspaceIcon name={action.icon} /> : null}
                  {action.label}
                </button>
              ))}
            </div>,
            portalContainer,
          )
        : null}
    </>
  );
}
