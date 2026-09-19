import { useState, useEffect, useCallback, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { bridge } from "../bridge";
import { formatBytes, formatDate, hasExtensionData, syncsAnything } from "../format";
import type { AppStatus, BackupSummary, Screen, SnapshotInfo } from "../types";
import ZenRunningGuard from "../components/ZenRunningGuard";
import RestoreResult from "../components/RestoreResult";
import { useConfirm } from "../components/ConfirmDialog";
import { useZenRunning } from "../hooks/useZenRunning";
import { useRestore } from "../hooks/useRestore";

interface Props {
  status: AppStatus;
  onStatusChange: (s: AppStatus) => void;
  onNavigate: (s: Screen) => void;
}

type Op = "idle" | "backing-up" | "success" | "error";

function Spinner() {
  return <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />;
}

export default function DashboardScreen({ status, onStatusChange, onNavigate }: Props) {
  const confirm = useConfirm();
  const zenRunning = useZenRunning();
  const restore = useRestore(status, onStatusChange);
  const [op, setOp] = useState<Op>("idle");
  const [progress, setProgress] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const [summary, setSummary] = useState<BackupSummary | null>(null);
  const [summaryError, setSummaryError] = useState<string | null>(null);
  // undefined while loading, null if there are no backups
  const [latest, setLatest] = useState<SnapshotInfo | null | undefined>(undefined);
  const [latestError, setLatestError] = useState<string | null>(null);

  const loadLatest = useCallback(() => {
    bridge
      .getSnapshots()
      .then((snaps) => {
        setLatest(snaps[0] ?? null);
        setLatestError(null);
      })
      .catch((e) => setLatestError(String(e)));
  }, []);

  useEffect(loadLatest, [loadLatest]);

  // Mods and extensions may have changed while Zen was open, or by a restore.
  useEffect(() => {
    bridge
      .getBackupSummary()
      .then((s) => {
        setSummary(s);
        setSummaryError(null);
      })
      .catch((e) => setSummaryError(String(e)));
  }, [zenRunning, restore.report]);

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
    restore.dismissReport();
    restore.dismissError();
    setOp("backing-up");
    setProgress("Checking for old snapshots…");
    setError(null);
    try {
      const legacyCount = await bridge.getLegacySnapshotCount();
      if (
        legacyCount > 0 &&
        !(await confirm({
          title: "Delete old snapshots?",
          message: (
            <>
              <p>
                Zen Sync now backs up only Sine mods and extension data.
                Everything else syncs through your Mozilla account in Zen.
              </p>
              <p className="mt-2">
                {legacyCount} old full-profile snapshot{legacyCount === 1 ? "" : "s"} from
                all your devices will be deleted from GitHub after this backup.
              </p>
            </>
          ),
          confirmLabel: "Back up and delete",
          danger: true,
        }))
      ) {
        setOp("idle");
        return;
      }
      await bridge.backupNow(legacyCount > 0);
      onStatusChange(await bridge.getStatus());
      loadLatest();
      setOp("success");
      setTimeout(() => setOp((o) => (o === "success" ? "idle" : o)), 3000);
    } catch (e) {
      setError(String(e));
      setOp("error");
    }
  }

  async function handleRestore() {
    if (zenRunning || !latest) return;
    setOp("idle");
    setProgress("");
    await restore.restore(latest);
  }

  const backingUp = op === "backing-up";
  const restoring = restore.restoringKey !== null;
  const options = status.syncOptions;
  const nothingSelected = !syncsAnything(options);
  const blocked = backingUp || restoring || zenRunning || nothingSelected;

  let latestLabel: string;
  if (latestError) latestLabel = "Couldn't load backups";
  else if (latest === undefined) latestLabel = "Checking…";
  else if (latest === null) latestLabel = "No backups yet";
  else latestLabel = `${latest.machineName} · ${formatDate(latest.pushedAt)}`;

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

      {zenRunning && <ZenRunningGuard action="backing up or restoring" />}

      {/* Actions */}
      <div className="flex flex-col gap-2">
        <button
          type="button"
          onClick={handleBackup}
          disabled={blocked}
          className="btn-primary w-full py-3 text-sm"
        >
          {backingUp ? (
            <>
              <Spinner />
              {progress || "Working…"}
            </>
          ) : (
            <>
              <span className="text-base">↑</span>
              Back up now
            </>
          )}
        </button>
        <button
          type="button"
          onClick={handleRestore}
          disabled={blocked || !latest}
          className="btn-secondary w-full py-2.5 text-sm"
        >
          {restoring ? (
            <>
              <Spinner />
              {progress || "Working…"}
            </>
          ) : (
            <span className="flex flex-col items-center gap-0.5 min-w-0 leading-tight">
              <span className="flex items-center gap-2">
                <span className="text-base">↓</span>
                Restore latest backup
              </span>
              <span className="text-xs font-normal text-muted truncate max-w-full">
                {latestLabel}
              </span>
            </span>
          )}
        </button>
        {nothingSelected && (
          <p className="text-xs text-muted text-center">
            Nothing is selected to sync.{" "}
            <button
              type="button"
              onClick={() => onNavigate("settings")}
              className="text-accent hover:text-accent-hover"
            >
              Choose in Settings
            </button>
          </p>
        )}
      </div>

      {/* Feedback */}
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
      {restore.report && (
        <RestoreResult report={restore.report} onDismiss={restore.dismissReport} />
      )}
      {restore.error && (
        <div className="p-3 bg-danger/10 border border-danger/20 rounded-xl text-sm text-danger animate-fade-in">
          <p className="font-medium">Restore failed</p>
          <p className="text-xs mt-1 opacity-80 whitespace-pre-line">{restore.error}</p>
          <button
            type="button"
            onClick={restore.dismissError}
            className="mt-2 text-xs underline opacity-70 hover:opacity-100"
          >
            Dismiss
          </button>
        </div>
      )}

      {/* Backup contents */}
      <div className="card p-3">
        <div className="flex items-center justify-between mb-2">
          <p className="section-label mb-0">What's synced</p>
          <button
            type="button"
            onClick={() => onNavigate("settings")}
            className="text-xs text-accent hover:text-accent-hover transition-colors"
          >
            Change
          </button>
        </div>
        {summary && (
          <div className="flex flex-col gap-1.5 text-xs">
            <SummaryRow label="Sine mods" on={options.sineMods}>
              {summary.sineInstalled || summary.modCount > 0
                ? `${summary.modCount}${summary.sineEngineVersion ? ` · Sine ${summary.sineEngineVersion}` : ""}`
                : "Sine not installed"}
            </SummaryRow>
            <SummaryRow label="Mod settings" on={options.modSettings}>
              {summary.modSettingCount}
            </SummaryRow>
            <SummaryRow label="Extensions" on={hasExtensionData(options)}>
              {summary.extensionCount}
              {options.extensionStorage && ` · ${formatBytes(summary.storageBytes)}`}
            </SummaryRow>
            <SummaryRow label="Zen shortcuts" on={options.zenShortcuts}>
              {summary.shortcutCount > 0 ? summary.shortcutCount : "None saved"}
            </SummaryRow>
            <SummaryRow label="about:config" on={options.aboutConfig}>
              {summary.prefCount === 1 ? "1 pref" : `${summary.prefCount} prefs`}
            </SummaryRow>
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

function SummaryRow({ label, on, children }: { label: string; on: boolean; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-muted">{label}</span>
      {on ? <span className="text-white">{children}</span> : <span className="text-muted">Off</span>}
    </div>
  );
}
