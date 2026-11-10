import { useLayoutEffect, type RefObject } from "react";

const activeModals: HTMLElement[] = [];

const FOCUSABLE =
  "button:not(:disabled), a[href], summary, input:not(:disabled), textarea:not(:disabled), select:not(:disabled), [tabindex]:not([tabindex='-1'])";

export function useModalFocus(root: RefObject<HTMLElement | null>, enabled = true) {
  useLayoutEffect(() => {
    if (!enabled) return;
    const dialog = root.current;
    if (!dialog) return;
    const previous =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    activeModals.push(dialog);
    dialog.focus({ preventScroll: true });
    const focusable = () =>
      Array.from(dialog.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
        (element) => {
          if (
            element.getAttribute("tabindex") === "-1" ||
            element.closest("[hidden]")
          )
            return false;
          const collapsed = element.closest("details:not([open])");
          if (
            collapsed &&
            collapsed.querySelector(":scope > summary") !== element
          )
            return false;
          return (
            element.getClientRects().length > 0 &&
            getComputedStyle(element).visibility !== "hidden"
          );
        },
      );
    const containFocus = (event: FocusEvent) => {
      if (activeModals.at(-1) !== dialog) return;
      if (event.target instanceof Node && !dialog.contains(event.target))
        dialog.focus({ preventScroll: true });
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (activeModals.at(-1) !== dialog || event.key !== "Tab") return;
      const items = focusable();
      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement;
      if (!first || !last) {
        event.preventDefault();
        dialog.focus();
        return;
      }
      if (event.shiftKey && (active === first || active === dialog)) {
        event.preventDefault();
        last.focus();
      } else if (
        !event.shiftKey &&
        (active === last || active === dialog || !dialog.contains(active))
      ) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("focusin", containFocus);
    document.addEventListener("keydown", onKeyDown, true);
    return () => {
      activeModals.splice(activeModals.indexOf(dialog), 1);
      document.removeEventListener("focusin", containFocus);
      document.removeEventListener("keydown", onKeyDown, true);
      if (previous?.isConnected) previous.focus({ preventScroll: true });
    };
  }, [root, enabled]);
}
