import { useState, useEffect, useCallback, useRef } from "react";
import { useRefreshOnFocus } from "../hooks/useRefreshOnFocus";
import { bridge } from "../bridge";
import { formatDate } from "../format";
import type { AppStatus, SnapshotInfo } from "../types";
import ZenRunningGuard from "../components/ZenRunningGuard";
import { useZenRunning } from "../hooks/useZenRunning";
import { snapshotKey, useOperations } from "../hooks/useOperations";

interface Props {
  status: AppStatus;
}

export default function SnapshotsScreen({ status }: Props) {
  const [snapshots, setSnapshots] = useState<SnapshotInfo[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const zenRunning = useZenRunning();
  const ops = useOperations();

  // Only the newest request may update the list; an older one finishing
  // later would put back a list without the latest snapshot.
  const requestRef = useRef(0);
  const load = useCallback(() => {
    const request = ++requestRef.current;
    setLoading(true);
    setLoadError(null);
    bridge
      .getSnapshots()
      .then((list) => request === requestRef.current && setSnapshots(list))
      .catch((e) => request === requestRef.current && setLoadError(String(e)))
      .finally(() => request === requestRef.current && setLoading(false));
  }, []);

  // Also re-run after a backup, which adds a snapshot, and when the window
  // regains focus, since other devices may have backed up meanwhile.
  useEffect(load, [load, ops.revision]);
  useRefreshOnFocus(load);

  return (
    <div className="p-5 flex flex-col gap-4 animate-fade-in">
      <div className="flex items-start justify-between gap-3">
        <div>
          <h2 className="text-sm font-semibold text-white">Backup history</h2>
          <p className="text-xs text-muted mt-0.5">
            Snapshots from all your devices, newest first.
          </p>
        </div>
        <button
          type="button"
          onClick={load}
          disabled={loading}
          className="text-xs text-accent hover:text-accent-hover disabled:opacity-40 transition-colors shrink-0"
        >
          Refresh
        </button>
      </div>

      {zenRunning && <ZenRunningGuard action="restoring a snapshot" />}

      {loadError && (
        <div role="alert" className="p-3 bg-danger/10 border border-danger/20 rounded-xl text-sm text-danger">
          <p className="font-medium">Couldn't load backups</p>
          <p className="text-xs mt-1 opacity-90 whitespace-pre-line select-text break-words">{loadError}</p>
          <button
            type="button"
            onClick={load}
            className="mt-2 text-xs underline opacity-80 hover:opacity-100"
          >
            Try again
          </button>
        </div>
      )}

      {loading && !snapshots && (
        <div className="flex items-center justify-center h-32" role="status" aria-label="Loading backups">
          <div className="w-5 h-5 border-2 border-accent border-t-transparent rounded-full animate-spin" />
        </div>
      )}

      {!loadError && snapshots && snapshots.length === 0 && (
        <div className="card p-6 text-center">
          <p className="text-subtle text-sm">No snapshots yet.</p>
          <p className="text-muted text-xs mt-1">
            Use “Back up now” on the Sync tab to create the first one.
          </p>
        </div>
      )}

      {snapshots && snapshots.length > 0 && (
        <ul className="flex flex-col gap-2">
          {snapshots.map((snap, i) => {
            const key = snapshotKey(snap);
            // The list is sorted newest first across all devices.
            const newest = i === 0;
            const thisDevice = status.machineId !== undefined && snap.machineId === status.machineId;
            return (
              <li
                key={key}
                className={`card p-3.5 flex items-center justify-between gap-3 transition-colors
                  ${newest ? "border-accent/30 bg-accent/5" : ""}`}
              >
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 mb-0.5">
                    <span className="text-sm font-medium text-white truncate" title={snap.machineName}>
                      {snap.machineName}
                    </span>
                    {thisDevice && <span className="badge-muted shrink-0">This device</span>}
                    {newest && <span className="badge-success shrink-0">Latest</span>}
                  </div>
                  <div className="flex items-center gap-2 text-xs text-muted">
                    <span>{formatDate(snap.pushedAt)}</span>
                    <span aria-hidden="true">·</span>
                    <span>{snap.sizeMb.toFixed(1)} MB</span>
                  </div>
                </div>
                <button
                  type="button"
                  onClick={() => ops.restore(snap)}
                  disabled={ops.busy || zenRunning}
                  aria-label={`Restore backup from ${snap.machineName}, ${formatDate(snap.pushedAt)}`}
                  className="btn-secondary text-xs px-3 py-1.5 shrink-0"
                >
                  {ops.restoringKey === key ? (
                    <>
                      <div className="w-3 h-3 border border-subtle border-t-transparent rounded-full animate-spin" />
                      Restoring…
                    </>
                  ) : (
                    "Restore"
                  )}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
