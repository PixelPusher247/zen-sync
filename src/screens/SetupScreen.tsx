import { useState } from "react";
import { bridge } from "../bridge";
import type { AppStatus } from "../types";

interface Props {
  onConnected: (status: AppStatus) => void;
}

export default function SetupScreen({ onConnected }: Props) {
  const [state, setState] = useState<"idle" | "connecting" | "error">("idle");
  const [error, setError] = useState<string | null>(null);

  async function handleConnect() {
    setState("connecting");
    setError(null);
    try {
      const status = await bridge.connectGithub();
      onConnected(status);
    } catch (e) {
      setError(String(e));
      setState("error");
    }
  }

  return (
    <div className="h-full flex flex-col items-center justify-center p-8 bg-surface animate-fade-in">
      <div className="w-full max-w-xs flex flex-col items-center gap-6">
        {/* Logo */}
        <div className="flex flex-col items-center gap-3">
          <div className="w-16 h-16 rounded-2xl bg-accent/15 flex items-center justify-center">
            <span className="text-accent text-2xl font-bold">Z</span>
          </div>
          <div className="text-center">
            <h1 className="text-xl font-bold text-white">Zen Sync</h1>
            <p className="text-muted text-sm mt-1">
              Encrypted Zen Browser profile backups
            </p>
          </div>
        </div>

        {/* Feature list */}
        <div className="w-full card p-4 flex flex-col gap-2.5">
          {[
            ["🔒", "AES-256-GCM encrypted backups"],
            ["🐙", "Stored in your private GitHub repo"],
            ["⚡", "Manual backup & restore, no auto-sync"],
            ["🛡️", "Device name & sync settings preserved"],
          ].map(([icon, text]) => (
            <div key={text} className="flex items-center gap-3">
              <span className="text-base">{icon}</span>
              <span className="text-subtle text-xs">{text}</span>
            </div>
          ))}
        </div>

        {/* OAuth setup note */}
        <div className="w-full p-3 bg-surface-overlay border border-surface-border rounded-xl text-xs text-muted leading-relaxed">
          <span className="text-accent font-medium">Before connecting:</span>{" "}
          create a GitHub OAuth App at{" "}
          <span className="font-mono text-subtle">
            github.com/settings/developers
          </span>{" "}
          with callback URL{" "}
          <span className="font-mono text-subtle">
            http://127.0.0.1
          </span>
          , then set{" "}
          <span className="font-mono text-subtle">GITHUB_CLIENT_ID</span> and{" "}
          <span className="font-mono text-subtle">GITHUB_CLIENT_SECRET</span>{" "}
          in your environment or build config.
        </div>

        {error && (
          <div className="w-full p-3 bg-danger/10 border border-danger/20 rounded-xl text-xs text-danger animate-fade-in">
            {error}
          </div>
        )}

        <button
          type="button"
          onClick={handleConnect}
          disabled={state === "connecting"}
          className="btn-primary w-full py-3 text-base"
        >
          {state === "connecting" ? (
            <>
              <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
              Connecting…
            </>
          ) : (
            <>
              <span>🐙</span>
              Connect to GitHub
            </>
          )}
        </button>

        <p className="text-muted text-xs text-center">
          A private{" "}
          <span className="font-mono text-subtle">zen-sync-backup</span>{" "}
          repository will be created in your GitHub account.
        </p>
      </div>
    </div>
  );
}
