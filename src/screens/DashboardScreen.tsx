import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { bridge } from "../bridge";
import { formatBytes } from "../format";
import type { AppStatus, BackupSummary } from "../types";
import ZenRunningGuard from "../components/ZenRunningGuard";

interface Props {
  status: AppStatus;
  onStatusChange: (s: AppStatus) => void;
}

type Op = "idle" | "backing-up" | "success" | "error";

function formatDate(iso: string | null): string {
  if (!iso) return "Never";
  try {
    const d = new Date(iso);
    return d.toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return iso;
  }
}

export default function DashboardScreen({ status, onStatusChange }: Props) {
  const [op, setOp] = useState<Op>("idle");
  const [progress, setProgress] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const [zenRunning, setZenRunning] = useState(false);
  const [summary, setSummary] = useState<BackupSummary | null>(null);
  const [summaryError, setSummaryError] = useState<string | null>(null);

  useEffect(() => {
    bridge.isZenRunning().then(setZenRunning).catch(() => {});
    bridge
      .getBackupSummary()
      .then(setSummary)
      .catch((e) => setSummaryError(String(e)));
  }, []);

  useEffect(() => {
    const unlisten = listen<string>("sync-progress", (e) =>
      setProgress(e.payload)
    );
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  async function handleBackup() {
    if (zenRunning) return;
    setOp("backing-up");
    setProgress("Checking for old snapshots…");
    setError(null);
    try {
      const legacyCount = await bridge.getLegacySnapshotCount();
      if (
        legacyCount > 0 &&
        !confirm(
          "Zen Sync now backs up only Sine mods and extension data. " +
            "Everything else syncs through your Mozilla account in Zen.\n\n" +
            `${legacyCount} old full-profile snapshot${legacyCount === 1 ? "" : "s"} ` +
            "from all your devices will be deleted from GitHub after this backup. Continue?"
        )
      ) {
        setOp("idle");
        return;
      }
      await bridge.backupNow(legacyCount > 0);
      const s = await bridge.getStatus();
      onStatusChange(s);
      setOp("success");
      setTimeout(() => setOp("idle"), 3000);
    } catch (e) {
      setError(String(e));
      setOp("error");
    }
  }

  const busy = op === "backing-up";

  return (
    <div className="p-5 flex flex-col gap-4 animate-fade-in">
      {/* Status card */}
      <div className="card p-4 flex flex-col gap-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <div className="w-2 h-2 rounded-full bg-success animate-pulse" />
            <span className="text-xs font-medium text-subtle">Connected</span>
          </div>
          <span className="text-xs text-muted">
            {status.username && `@${status.username}`}
          </span>
        </div>
        <div className="divider" />
        <div className="grid grid-cols-2 gap-3">
          <div>
            <p className="section-label">This device</p>
            <p className="text-sm font-semibold text-white truncate">
              {status.machineName}
            </p>
          </div>
          <div>
            <p className="section-label">Last backup</p>
            <p className="text-sm font-semibold text-white">
              {formatDate(status.lastBackupAt)}
            </p>
          </div>
        </div>
        {status.snapshotCount > 0 && (
          <>
            <div className="divider" />
            <div className="flex items-center justify-between text-xs">
              <span className="text-muted">Snapshots stored</span>
              <span className="badge-muted">{status.snapshotCount}</span>
            </div>
          </>
        )}
      </div>

      {/* Zen running guard */}
      {zenRunning && <ZenRunningGuard action="backing up" />}

      {/* Backup button */}
      {!zenRunning && (
        <div className="flex flex-col gap-2">
          <button
            type="button"
            onClick={handleBackup}
            disabled={busy}
            className="btn-primary w-full py-3 text-sm relative overflow-hidden"
          >
            {busy ? (
              <>
                <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                {progress || "Working…"}
              </>
            ) : (
              <>
                <span className="text-base">↑</span>
                Backup Now
              </>
            )}
          </button>
        </div>
      )}

      {/* Success / error feedback */}
      {op === "success" && (
        <div className="flex items-center gap-2 p-3 bg-success/10 border border-success/20 rounded-xl text-sm text-success animate-fade-in">
          <span>✓</span>
          <span>Backup complete</span>
        </div>
      )}
      {op === "error" && error && (
        <div className="p-3 bg-danger/10 border border-danger/20 rounded-xl text-sm text-danger animate-fade-in">
          <p className="font-medium">Backup failed</p>
          <p className="text-xs mt-1 opacity-80">{error}</p>
          <button
            type="button"
            onClick={() => setOp("idle")}
            className="mt-2 text-xs underline opacity-70 hover:opacity-100"
          >
            Dismiss
          </button>
        </div>
      )}

      {/* Backup contents */}
      <div className="card p-3">
        <p className="section-label">What's backed up</p>
        {summary && (
          <div className="flex flex-col gap-1.5 text-xs">
            <div className="flex items-center justify-between">
              <span className="text-muted">Sine mods</span>
              <span className="text-white">
                {summary.sineInstalled || summary.modCount > 0
                  ? `${summary.modCount}${summary.sineEngineVersion ? ` · Sine ${summary.sineEngineVersion}` : ""}`
                  : "Sine not installed"}
              </span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-muted">Mod settings</span>
              <span className="text-white">{summary.modSettingCount}</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-muted">Extensions</span>
              <span className="text-white">
                {summary.extensionCount} · {formatBytes(summary.storageBytes)}
              </span>
            </div>
          </div>
        )}
        {summaryError && <p className="text-xs text-danger">{summaryError}</p>}
        <p className="text-xs text-muted mt-2">
          Spaces, containers, bookmarks and other browser data sync through your
          Mozilla account in Zen.
        </p>
      </div>
    </div>
  );
}
