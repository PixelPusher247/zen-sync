import { useCallback, useEffect, useRef, useState } from "react";

export type SaveState = "idle" | "saving" | "saved" | "error";

// Saves outlive the picker screens that start them. These let other screens
// wait for them and hear about failures nobody was left to see.
const inFlight = new Set<Promise<void>>();
const failureListeners = new Set<(message: string) => void>();
// A failure that happened while nobody was listening, delivered to the next.
let unseenFailure: string | null = null;

/** Resolves once every save started so far has finished. */
export function allSaved(): Promise<void> {
  return Promise.all([...inFlight]).then(() => undefined);
}

/** Called with the error of a save that failed after its screen was closed. */
export function onBackgroundSaveFailure(listener: (message: string) => void): () => void {
  failureListeners.add(listener);
  if (unseenFailure) {
    listener(unseenFailure);
    unseenFailure = null;
  }
  return () => {
    failureListeners.delete(listener);
  };
}

function reportBackgroundFailure(message: string) {
  if (failureListeners.size === 0) unseenFailure = message;
  failureListeners.forEach((listener) => listener(message));
}

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
    const job = (async () => {
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
      if (!mounted.current) {
        if (failure) reportBackgroundFailure(failure);
        return;
      }
      setError(failure);
      setState(failure ? "error" : "saved");
    })();
    inFlight.add(job);
    void job.finally(() => inFlight.delete(job));
  }, []);

  return { state, error, push };
}
