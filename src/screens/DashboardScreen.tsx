import { useState, useEffect, useCallback, useRef, type ReactNode } from "react";
import { useRefreshOnFocus, useTick } from "../hooks/useRefreshOnFocus";
import { bridge } from "../bridge";
import {
  formatBytes,
  formatDate,
  formatRelative,
  hasExtensionData,
  syncsAnything,
} from "../format";
import type { AppStatus, BackupSummary, Screen, SnapshotInfo } from "../types";
import ZenRunningGuard from "../components/ZenRunningGuard";
import { useZenRunning } from "../hooks/useZenRunning";
import { useOperations } from "../hooks/useOperations";

interface Props {
  status: AppStatus;
  onNavigate: (s: Screen) => void;
}

function Spinner() {
  return <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />;
}

export default function DashboardScreen({ status, onNavigate }: Props) {
  const zenRunning = useZenRunning();
  const ops = useOperations();
  const [summary, setSummary] = useState<BackupSummary | null>(null);
  const [summaryError, setSummaryError] = useState<string | null>(null);
  // undefined while loading, null if there are no backups
  const [latest, setLatest] = useState<SnapshotInfo | null | undefined>(undefined);
  const [latestError, setLatestError] = useState<string | null>(null);

  // Only the newest request may update "latest"; see SnapshotsScreen.
  const requestRef = useRef(0);
  const loadLatest = useCallback(() => {
    const request = ++requestRef.current;
    setLatestError(null);
    bridge
      .getSnapshots()
      .then((snaps) => request === requestRef.current && setLatest(snaps[0] ?? null))
      .catch((e) => request === requestRef.current && setLatestError(String(e)));
  }, []);

  // Also re-run after a backup, which adds a snapshot, and when the window
  // regains focus, so "Restore latest backup" names what is really latest.
  useEffect(loadLatest, [loadLatest, ops.revision]);
  useRefreshOnFocus(loadLatest);
  // Keeps "3 minutes ago" current while the window stays open.
  useTick(60_000);

  // Mods and extensions may have changed while Zen was open, or by a restore.
  useEffect(() => {
    bridge
      .getBackupSummary()
      .then((s) => {
        setSummary(s);
        setSummaryError(null);
      })
      .catch((e) => setSummaryError(String(e)));
  }, [zenRunning, ops.restoreReport]);

  const backingUp = ops.backupState === "running";
  const restoring = ops.restoringKey !== null;
  const options = status.syncOptions;
  const nothingSelected = !syncsAnything(options);
  const blocked = ops.busy || zenRunning || nothingSelected;

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
            <div className="w-2 h-2 rounded-full bg-success" />
            <span className="text-xs font-medium text-subtle">Connected to GitHub</span>
          </div>
          <span className="text-xs text-muted">
            {status.username && `@${status.username}`}
          </span>
        </div>
        <div className="divider" />
        <div className="grid grid-cols-2 gap-3">
          <div className="min-w-0">
            <p className="section-label">This device</p>
            <p className="text-sm font-semibold text-white truncate" title={status.machineName}>
              {status.machineName}
            </p>
          </div>
          <div className="min-w-0">
            <p className="section-label">Last backup</p>
            <p
              className="text-sm font-semibold text-white truncate"
              title={status.lastBackupAt ? formatDate(status.lastBackupAt) : undefined}
            >
              {formatRelative(status.lastBackupAt)}
            </p>
          </div>
        </div>
        <div className="divider" />
        <div className="flex items-center justify-between text-xs">
          <span className="text-muted">Snapshots kept per device</span>
          <span className="badge-muted">{status.snapshotCount}</span>
        </div>
      </div>

      {zenRunning && <ZenRunningGuard action="backing up or restoring" />}

      {/* Actions */}
      <div className="flex flex-col gap-2">
        <button
          type="button"
          onClick={ops.backup}
          disabled={blocked}
          className="btn-primary w-full py-3 text-sm"
        >
          {backingUp ? (
            <>
              <Spinner />
              Backing up…
            </>
          ) : (
            <>
              <span className="text-base" aria-hidden="true">↑</span>
              Back up now
            </>
          )}
        </button>
        <button
          type="button"
          onClick={() => latest && ops.restore(latest)}
          disabled={blocked || !latest}
          className="btn-secondary w-full py-2.5 text-sm"
        >
          {restoring ? (
            <>
              <Spinner />
              Restoring…
            </>
          ) : (
            <span className="flex flex-col items-center gap-0.5 min-w-0 leading-tight">
              <span className="flex items-center gap-2">
                <span className="text-base" aria-hidden="true">↓</span>
                Restore latest backup
              </span>
              <span className="text-xs font-normal text-muted truncate max-w-full">
                {latestLabel}
              </span>
            </span>
          )}
        </button>
        {latestError && (
          <p className="text-xs text-muted text-center">
            <span className="select-text">{latestError}</span>{" "}
            <button
              type="button"
              onClick={loadLatest}
              className="text-accent hover:text-accent-hover"
            >
              Try again
            </button>
          </p>
        )}
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
        {summaryError && <p className="text-xs text-danger select-text break-words">{summaryError}</p>}
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
