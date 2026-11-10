import { useCallback, useEffect, useRef, useState } from "react";

// A modal answer belongs to exactly one request. Dismissal and unmount cancel it.
export function useConfirmation<T>() {
  const [pending, setPending] = useState<T | null>(null);
  const resolveRef = useRef<((accepted: boolean) => void) | null>(null);
  const request = useCallback((value: T) => new Promise<boolean>((resolve) => {
    if (resolveRef.current) { resolve(false); return; }
    resolveRef.current = resolve;
    setPending(value);
  }), []);
  const answer = useCallback((accepted: boolean) => {
    const resolve = resolveRef.current;
    resolveRef.current = null;
    setPending(null);
    resolve?.(accepted);
  }, []);
  useEffect(() => () => {
    resolveRef.current?.(false);
    resolveRef.current = null;
  }, []);
  return { pending, request, answer };
}
