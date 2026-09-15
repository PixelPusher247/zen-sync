import { useState, useEffect } from "react";
import { bridge } from "../bridge";
import { formatDate } from "../format";
import type { AppStatus, SnapshotInfo } from "../types";
import ZenRunningGuard from "../components/ZenRunningGuard";
import RestoreResult from "../components/RestoreResult";
import { useZenRunning } from "../hooks/useZenRunning";
import { snapshotKey, useRestore } from "../hooks/useRestore";

interface Props {
  status: AppStatus;
  onStatusChange: (s: AppStatus) => void;
}

export default function SnapshotsScreen({ status, onStatusChange }: Props) {
  const [snapshots, setSnapshots] = useState<SnapshotInfo[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const zenRunning = useZenRunning();
  const restore = useRestore(status, onStatusChange);

  useEffect(() => {
    bridge
      .getSnapshots()
      .then(setSnapshots)
      .catch((e) => setLoadError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-32">
        <div className="w-5 h-5 border-2 border-accent border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  const error = restore.error ?? loadError;

  return (
    <div className="p-5 flex flex-col gap-4 animate-fade-in">
      <div>
        <h2 className="text-sm font-semibold text-white">Backup History</h2>
        <p className="text-xs text-muted mt-0.5">
          Restore any previous snapshot from your GitHub backup repo.
        </p>
      </div>

      {zenRunning && <ZenRunningGuard action="restoring a snapshot" />}

      {error && (
        <div className="p-3 bg-danger/10 border border-danger/20 rounded-xl text-sm text-danger">
          <p className="font-medium">Error</p>
          <p className="text-xs mt-1 opacity-80 whitespace-pre-line">{error}</p>
          <button
            type="button"
            onClick={() => {
              restore.dismissError();
              setLoadError(null);
            }}
            className="mt-2 text-xs underline opacity-70 hover:opacity-100"
          >
            Dismiss
          </button>
        </div>
      )}

      {restore.report && (
        <RestoreResult report={restore.report} onDismiss={restore.dismissReport} />
      )}

      {snapshots && snapshots.length === 0 && (
        <div className="card p-6 text-center">
          <p className="text-muted text-sm">No snapshots yet.</p>
          <p className="text-muted text-xs mt-1">
            Back up your mods and extension data to create a snapshot.
          </p>
        </div>
      )}

      {snapshots && snapshots.length > 0 && (
        <div className="flex flex-col gap-2">
          {snapshots.map((snap) => {
            const key = snapshotKey(snap);
            return (
              <div
                key={key}
                className={`card p-3.5 flex items-center justify-between gap-3 transition-colors
                  ${snap.isCurrent ? "border-accent/30 bg-accent/5" : ""}`}
              >
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 mb-0.5">
                    <span className="text-sm font-medium text-white truncate">
                      {snap.machineName}
                    </span>
                    {snap.isCurrent && (
                      <span className="badge-success">Latest</span>
                    )}
                  </div>
                  <div className="flex items-center gap-3 text-xs text-muted">
                    <span>{formatDate(snap.pushedAt)}</span>
                    <span>·</span>
                    <span>{snap.sizeMb.toFixed(1)} MB</span>
                  </div>
                </div>
                <button
                  type="button"
                  onClick={() => restore.restore(snap)}
                  disabled={restore.restoringKey !== null || zenRunning}
                  className="btn-secondary text-xs px-3 py-1.5 shrink-0"
                >
                  {restore.restoringKey === key ? (
                    <div className="w-3 h-3 border border-muted border-t-transparent rounded-full animate-spin" />
                  ) : (
                    "Restore"
                  )}
                </button>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
