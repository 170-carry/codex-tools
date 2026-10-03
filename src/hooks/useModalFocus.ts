import { useLayoutEffect, type RefObject } from "react";

const FOCUSABLE =
  "button:not(:disabled), a[href], summary, input:not(:disabled), textarea:not(:disabled), select:not(:disabled), [tabindex]:not([tabindex='-1'])";

export function useModalFocus(root: RefObject<HTMLElement | null>) {
  useLayoutEffect(() => {
    const dialog = root.current;
    if (!dialog) return;
    const previous =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
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
      if (event.target instanceof Node && !dialog.contains(event.target))
        dialog.focus({ preventScroll: true });
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Tab") return;
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
      document.removeEventListener("focusin", containFocus);
      document.removeEventListener("keydown", onKeyDown, true);
      if (previous?.isConnected) previous.focus({ preventScroll: true });
    };
  }, [root]);
}
