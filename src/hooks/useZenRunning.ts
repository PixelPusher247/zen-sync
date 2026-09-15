import { useEffect, useState } from "react";
import { bridge } from "../bridge";

const POLL_MS = 2000;

/** Whether Zen Browser is open, re-checked every few seconds while the window is visible. */
export function useZenRunning(): boolean {
  const [running, setRunning] = useState(false);

  useEffect(() => {
    let active = true;
    const check = () => {
      if (document.hidden) return;
      bridge
        .isZenRunning()
        .then((r) => active && setRunning(r))
        .catch(() => {});
    };
    check();
    const interval = setInterval(check, POLL_MS);
    document.addEventListener("visibilitychange", check);
    return () => {
      active = false;
      clearInterval(interval);
      document.removeEventListener("visibilitychange", check);
    };
  }, []);

  return running;
}
