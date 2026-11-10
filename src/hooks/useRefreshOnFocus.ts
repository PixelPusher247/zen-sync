import { useEffect, useRef, useState } from "react";

/** Calls `refresh` whenever the window comes back into view. */
export function useRefreshOnFocus(refresh: () => void) {
  const refreshRef = useRef(refresh);
  refreshRef.current = refresh;

  useEffect(() => {
    const onVisible = () => {
      if (!document.hidden) refreshRef.current();
    };
    window.addEventListener("focus", onVisible);
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      window.removeEventListener("focus", onVisible);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, []);
}

/** Re-renders every `ms` milliseconds, for text like "3 minutes ago". */
export function useTick(ms: number) {
  const [, setTick] = useState(0);
  useEffect(() => {
    const id = setInterval(() => setTick((t) => t + 1), ms);
    return () => clearInterval(id);
  }, [ms]);
}
