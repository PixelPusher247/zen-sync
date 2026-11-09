import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { listen } from "@tauri-apps/api/event";
import { bridge } from "../bridge";
import { useConfirm } from "../components/ConfirmDialog";
import { describeSynced, formatDate } from "../format";
import type { AppStatus, RestoreReport, SnapshotInfo } from "../types";

export function snapshotKey(snap: SnapshotInfo): string {
  return `${snap.machineId}-${snap.index}`;
}

type BackupState = "idle" | "running" | "success" | "error";

interface Operations {
  /** A backup or restore is in flight; nothing else may start. */
  busy: boolean;
  /** Latest "sync-progress" message of the running operation. */
  progress: string;
  backupState: BackupState;
  backupError: string | null;
  /** Key of the snapshot being restored, if any. */
  restoringKey: string | null;
  restoreReport: RestoreReport | null;
  restoreError: string | null;
  /** Bumps whenever the snapshot list on GitHub may have changed. */
  revision: number;
  backup: () => Promise<void>;
  restore: (snap: SnapshotInfo) => Promise<void>;
  dismissBackup: () => void;
  dismissRestore: () => void;
}

const OperationsContext = createContext<Operations | null>(null);

export function useOperations(): Operations {
  const ops = useContext(OperationsContext);
  if (!ops) throw new Error("useOperations must be used inside OperationsProvider");
  return ops;
}

interface ProviderProps {
  status: AppStatus;
  onStatusChange: (s: AppStatus) => void;
  children: ReactNode;
}

/**
 * Owns backup and restore so their progress and outcome survive switching
 * screens, and so a second operation can't start while one is running.
 */
export function OperationsProvider({ status, onStatusChange, children }: ProviderProps) {
  const confirm = useConfirm();
  const [progress, setProgress] = useState("");
  const [backupState, setBackupState] = useState<BackupState>("idle");
  const [backupError, setBackupError] = useState<string | null>(null);
  const [restoringKey, setRestoringKey] = useState<string | null>(null);
  const [restoreReport, setRestoreReport] = useState<RestoreReport | null>(null);
  const [restoreError, setRestoreError] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  // Guards against a second start before the first re-render disables the buttons.
  const busyRef = useRef(false);
  const syncOptions = status.syncOptions;

  useEffect(() => {
    const unlisten = listen<string>("sync-progress", (e) => setProgress(e.payload));
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const dismissBackup = useCallback(() => {
    setBackupState((s) => (s === "running" ? s : "idle"));
    setBackupError(null);
  }, []);

  const dismissRestore = useCallback(() => {
    setRestoreReport(null);
    setRestoreError(null);
  }, []);

  const backup = useCallback(async () => {
    if (busyRef.current) return;
    busyRef.current = true;
    setRestoreReport(null);
    setRestoreError(null);
    setBackupError(null);
    setBackupState("running");
    setProgress("Checking for old snapshots…");
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
        setBackupState("idle");
        return;
      }
      await bridge.backupNow(legacyCount > 0);
      onStatusChange(await bridge.getStatus());
      setRevision((r) => r + 1);
      setBackupState("success");
      setTimeout(() => setBackupState((s) => (s === "success" ? "idle" : s)), 4000);
    } catch (e) {
      setBackupError(String(e));
      setBackupState("error");
    } finally {
      busyRef.current = false;
    }
  }, [confirm, onStatusChange]);

  const restore = useCallback(
    async (snap: SnapshotInfo) => {
      if (busyRef.current) return;
      busyRef.current = true;
      try {
        const ok = await confirm({
          title: "Restore this backup?",
          message: (
            <>
              <p>
                Your {describeSynced(syncOptions)} on this device will be
                replaced with the backup from{" "}
                <span className="text-white">{snap.machineName}</span> (
                {formatDate(snap.pushedAt)}).
              </p>
              <p className="mt-2">
                The current files are saved locally first, so the restore can
                be rolled back by hand.
              </p>
            </>
          ),
          confirmLabel: "Restore",
          cautious: true,
        });
        if (!ok) return;

        setBackupState("idle");
        setBackupError(null);
        setRestoreError(null);
        setRestoreReport(null);
        setProgress("Starting restore…");
        setRestoringKey(snapshotKey(snap));
        const result = await bridge.restoreSnapshot(snap.index, snap.machineId);
        onStatusChange(await bridge.getStatus());
        setRestoreReport(result);
        if (result.warnings.length === 0) {
          setTimeout(() => setRestoreReport((r) => (r === result ? null : r)), 8000);
        }
      } catch (e) {
        setRestoreError(String(e));
      } finally {
        setRestoringKey(null);
        busyRef.current = false;
      }
    },
    [confirm, onStatusChange, syncOptions],
  );

  const value = useMemo<Operations>(
    () => ({
      busy: backupState === "running" || restoringKey !== null,
      progress,
      backupState,
      backupError,
      restoringKey,
      restoreReport,
      restoreError,
      revision,
      backup,
      restore,
      dismissBackup,
      dismissRestore,
    }),
    [
      progress,
      backupState,
      backupError,
      restoringKey,
      restoreReport,
      restoreError,
      revision,
      backup,
      restore,
      dismissBackup,
      dismissRestore,
    ],
  );

  return <OperationsContext.Provider value={value}>{children}</OperationsContext.Provider>;
}
