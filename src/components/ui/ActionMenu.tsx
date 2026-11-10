import { Presence } from "./Presence";
import { FadingSurface } from "./FadingSurface";
import { createPortal } from "react-dom";
import { useId } from "react";
import type { ReactNode } from "react";
import { useAnchoredMenu } from "../../hooks/useAnchoredMenu";
export type MenuAction = { label: string; action: () => void; disabled?: boolean; danger?: boolean; icon?: ReactNode };
export function ActionMenu({ label, items, triggerLabel, disabled }: { label: string; items: MenuAction[]; triggerLabel?: string; disabled?: boolean }) {
  const { open, toggle, close, triggerRef, menuRef, position, portalContainer } = useAnchoredMenu();
  const id = useId();
  return <><button ref={triggerRef} type="button" className={triggerLabel ? "ghost" : "rowMoreButton"} disabled={disabled} title={label} aria-label={label} aria-haspopup="menu" aria-expanded={open} aria-controls={open ? id : undefined} onClick={toggle}>{triggerLabel ?? "···"}</button>
  <Presence present={open}>{open && portalContainer && createPortal(<FadingSurface ref={menuRef} id={id} className="accountActionMenu" style={position} role="menu" aria-label={label}>{items.map((item) => <button key={item.label} type="button" role="menuitem" disabled={item.disabled} className={item.danger ? "dangerMenuItem" : undefined} onClick={() => { close(true); item.action(); }}>{item.icon}<span>{item.label}</span></button>)}</FadingSurface>, portalContainer)}</Presence></>;
}
