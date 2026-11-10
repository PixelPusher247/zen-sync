import { useRef, useState } from "react";
import { bridge } from "../bridge";
import type { AppStatus } from "../types";
import { FloatingTitleBar } from "../components/WindowControls";

interface Props {
  onConnected: (status: AppStatus) => void;
  /** A saved sign-in is being restored, e.g. waiting for the network. */
  reconnecting?: boolean;
}

const POINTS = [
  "Encrypted on this device before anything is uploaded",
  "Stored in a private repository in your GitHub account",
  "Runs only when you click — no background sync",
  "Bookmarks, passwords and history stay with Mozilla sync",
];

function GitHubMark() {
  return (
    <svg viewBox="0 0 16 16" className="w-4 h-4" fill="currentColor" aria-hidden="true">
      <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27s1.36.09 2 .27c1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8Z" />
    </svg>
  );
}

export default function SetupScreen({ onConnected, reconnecting = false }: Props) {
  const [state, setState] = useState<"idle" | "connecting" | "error">("idle");
  const [error, setError] = useState<string | null>(null);
  // Only the newest attempt may report a failure; a cancelled one stays quiet.
  const attempt = useRef(0);

  async function handleConnect() {
    const mine = ++attempt.current;
    setState("connecting");
    setError(null);
    try {
      const status = await bridge.connectGithub();
      // A sign-in finished after "Cancel" still counts: the user did connect.
      onConnected(status);
    } catch (e) {
      if (attempt.current !== mine) return;
      setError(String(e));
      setState("error");
    }
  }

  function handleCancel() {
    attempt.current++;
    setState("idle");
  }

  return (
    <div className="relative h-full flex flex-col items-center justify-center p-8 bg-surface animate-fade-in">
      <FloatingTitleBar />
      <div className="w-full max-w-xs flex flex-col items-center gap-6">
        {/* Logo */}
        <div className="flex flex-col items-center gap-3">
          <img src="/app-icon.svg" alt="" className="w-16 h-16 rounded-2xl" draggable={false} />
          <div className="text-center">
            <h1 className="text-xl font-bold text-white">Zen Sync</h1>
            <p className="text-subtle text-sm mt-1">
              Encrypted backups of your Zen mods and extension data
            </p>
          </div>
        </div>

        <ul className="w-full card p-4 flex flex-col gap-2.5">
          {POINTS.map((text) => (
            <li key={text} className="flex items-start gap-2.5">
              <span className="text-accent text-xs leading-[18px]" aria-hidden="true">✓</span>
              <span className="text-subtle text-xs">{text}</span>
            </li>
          ))}
        </ul>

        {reconnecting && state === "idle" && (
          <div
            role="status"
            className="w-full flex items-center gap-2.5 p-3 bg-accent/10 border border-accent/20 rounded-xl text-xs text-white animate-fade-in"
          >
            <div className="w-3.5 h-3.5 shrink-0 border-2 border-accent/30 border-t-accent rounded-full animate-spin" />
            <span>
              Reconnecting to GitHub with your saved sign-in. This retries for a
              few minutes while the network comes up.
            </span>
          </div>
        )}

        {error && (
          <div role="alert" className="w-full p-3 bg-danger/10 border border-danger/20 rounded-xl text-xs text-danger animate-fade-in">
            <p className="font-medium">Couldn't connect to GitHub</p>
            <p className="mt-1 opacity-90 select-text break-words">{error}</p>
          </div>
        )}

        <div className="w-full flex flex-col items-center gap-2">
          <button
            type="button"
            onClick={handleConnect}
            disabled={state === "connecting"}
            className="btn-primary w-full py-3 text-base"
          >
            {state === "connecting" ? (
              <>
                <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                Waiting for GitHub…
              </>
            ) : (
              <>
                <GitHubMark />
                {state === "error" ? "Try again" : "Connect to GitHub"}
              </>
            )}
          </button>

          {state === "connecting" ? (
            <p className="text-subtle text-xs text-center animate-fade-in" role="status">
              Finish signing in in the browser window that just opened.{" "}
              <button
                type="button"
                onClick={handleCancel}
                className="text-accent hover:text-accent-hover"
              >
                Cancel
              </button>
            </p>
          ) : (
            <p className="text-muted text-xs text-center">
              Creates a private{" "}
              <span className="font-mono text-subtle">zen-sync-backup</span>{" "}
              repository in your GitHub account.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
