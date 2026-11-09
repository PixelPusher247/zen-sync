import { useCallback, useEffect, useRef, useState } from "react";

export type SaveState = "idle" | "saving" | "saved" | "error";

/**
 * Saves every pushed value, one request at a time, always ending on the
 * newest one. A save still in flight when the screen closes runs to the end.
 */
export function useAutosave<T>(save: (value: T) => Promise<void>) {
  const [state, setState] = useState<SaveState>("idle");
  const [error, setError] = useState<string | null>(null);
  const saveRef = useRef(save);
  saveRef.current = save;
  const pending = useRef<{ value: T } | null>(null);
  const running = useRef(false);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const push = useCallback((value: T) => {
    pending.current = { value };
    if (running.current) return;
    running.current = true;
    setState("saving");
    setError(null);
    void (async () => {
      let failure: string | null = null;
      while (pending.current) {
        const next = pending.current;
        pending.current = null;
        try {
          await saveRef.current(next.value);
          failure = null;
        } catch (e) {
          failure = String(e);
        }
      }
      running.current = false;
      if (!mounted.current) return;
      setError(failure);
      setState(failure ? "error" : "saved");
    })();
  }, []);

  return { state, error, push };
}
