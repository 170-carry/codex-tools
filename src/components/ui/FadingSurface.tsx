import type { HTMLAttributes, Ref } from "react";
import { useExiting } from "./presenceContext";
export function FadingSurface({ ref, ...props }: HTMLAttributes<HTMLDivElement> & { ref?: Ref<HTMLDivElement> }) {
  const exiting = useExiting();
  return <div {...props} ref={ref} data-exiting={exiting || undefined} inert={exiting} />;
}
