import { useState } from "react";
import { bridge } from "../bridge";
import { useConfirm } from "../components/ConfirmDialog";
import { describeSynced, formatDate } from "../format";
import type { AppStatus, RestoreReport, SnapshotInfo } from "../types";

export function snapshotKey(snap: SnapshotInfo): string {
  return `${snap.machineId}-${snap.index}`;
}

/** Confirm-then-restore flow shared by the dashboard and the history screen. */
export function useRestore(status: AppStatus, onStatusChange: (s: AppStatus) => void) {
  const confirm = useConfirm();
  const [restoringKey, setRestoringKey] = useState<string | null>(null);
  const [report, setReport] = useState<RestoreReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function restore(snap: SnapshotInfo) {
    const ok = await confirm({
      title: "Restore this backup?",
      message: (
        <>
          <p>
            Your {describeSynced(status.syncOptions)} on this device will be
            replaced with the backup from{" "}
            <span className="text-white">{snap.machineName}</span> (
            {formatDate(snap.pushedAt)}).
          </p>
          <p className="mt-2">The current files are saved locally first.</p>
        </>
      ),
      confirmLabel: "Restore",
    });
    if (!ok) return;

    setRestoringKey(snapshotKey(snap));
    setError(null);
    setReport(null);
    try {
      const result = await bridge.restoreSnapshot(snap.index, snap.machineId);
      onStatusChange(await bridge.getStatus());
      setReport(result);
      if (result.warnings.length === 0) {
        setTimeout(() => setReport((r) => (r === result ? null : r)), 4000);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setRestoringKey(null);
    }
  }

  return {
    restore,
    restoringKey,
    report,
    error,
    dismissReport: () => setReport(null),
    dismissError: () => setError(null),
  };
}
