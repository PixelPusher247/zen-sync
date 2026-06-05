import { useState, useEffect } from "react";
import { bridge } from "../bridge";
import type { AppStatus, SnapshotInfo } from "../types";
import ZenRunningGuard from "../components/ZenRunningGuard";

interface Props {
  onStatusChange: (s: AppStatus) => void;
}

function formatDate(iso: string): string {
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

export default function SnapshotsScreen({ onStatusChange }: Props) {
  const [snapshots, setSnapshots] = useState<SnapshotInfo[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [restoringKey, setRestoringKey] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [restored, setRestored] = useState<string | null>(null);
  const [zenRunning, setZenRunning] = useState(false);

  useEffect(() => {
    Promise.all([
      bridge.getSnapshots(),
      bridge.isZenRunning(),
    ])
      .then(([snaps, zen]) => {
        setSnapshots(snaps);
        setZenRunning(zen);
      })
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  async function handleRestore(index: number, machineId: string) {
    if (zenRunning) return;
    const key = `${machineId}-${index}`;
    setRestoringKey(key);
    setError(null);
    try {
      await bridge.restoreSnapshot(index, machineId);
      const s = await bridge.getStatus();
      onStatusChange(s);
      setRestored(key);
      setTimeout(() => setRestored(null), 4000);
    } catch (e) {
      setError(String(e));
    } finally {
      setRestoringKey(null);
    }
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center h-32">
        <div className="w-5 h-5 border-2 border-accent border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

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
          <p className="text-xs mt-1 opacity-80">{error}</p>
          <button
            type="button"
            onClick={() => setError(null)}
            className="mt-2 text-xs underline opacity-70 hover:opacity-100"
          >
            Dismiss
          </button>
        </div>
      )}

      {restored !== null && (
        <div className="flex items-center gap-2 p-3 bg-success/10 border border-success/20 rounded-xl text-sm text-success animate-fade-in">
          <span>✓</span>
          <span>Snapshot restored successfully</span>
        </div>
      )}

      {snapshots && snapshots.length === 0 && (
        <div className="card p-6 text-center">
          <p className="text-muted text-sm">No snapshots yet.</p>
          <p className="text-muted text-xs mt-1">
            Back up your profile to create a snapshot.
          </p>
        </div>
      )}

      {snapshots && snapshots.length > 0 && (
        <div className="flex flex-col gap-2">
          {snapshots.map((snap) => {
            const snapKey = `${snap.machineId}-${snap.index}`;
            return (
              <div
                key={snapKey}
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
                  onClick={() => handleRestore(snap.index, snap.machineId)}
                  disabled={restoringKey !== null || zenRunning}
                  className="btn-secondary text-xs px-3 py-1.5 shrink-0"
                >
                  {restoringKey === snapKey ? (
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
