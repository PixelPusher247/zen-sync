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
import { getCurrentWindow } from "@tauri-apps/api/window";
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
  // A backup or restore is actually running in the backend (not just asking
  // for confirmation).
  const runningRef = useRef(false);
  // Which backup a "Backup complete" timer belongs to.
  const backupRunRef = useRef(0);
  const syncOptions = status.syncOptions;

  useEffect(() => {
    const unlisten = listen<string>("sync-progress", (e) => setProgress(e.payload));
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // Closing the window quits the app (there is no tray), which would cut a
  // restore off half-way. Ask first while something is running; this covers
  // the ✕ button and Alt+F4 alike.
  useEffect(() => {
    const window = getCurrentWindow();
    const unlisten = window.onCloseRequested(async (event) => {
      if (!runningRef.current) return;
      event.preventDefault();
      const quit = await confirm({
        title: "Quit while this is running?",
        message:
          "A backup or restore is still running. Quitting now can leave it unfinished.",
        confirmLabel: "Quit anyway",
        danger: true,
      });
      if (quit) await window.destroy();
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, [confirm]);

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
      runningRef.current = true;
      await bridge.backupNow(legacyCount > 0);
      runningRef.current = false;
      onStatusChange(await bridge.getStatus());
      setRevision((r) => r + 1);
      setBackupState("success");
      const run = ++backupRunRef.current;
      setTimeout(() => {
        // Only clear this backup's message, not a later one's.
        if (backupRunRef.current === run) {
          setBackupState((s) => (s === "success" ? "idle" : s));
        }
      }, 4000);
    } catch (e) {
      setBackupError(String(e));
      setBackupState("error");
    } finally {
      runningRef.current = false;
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
                The current files are saved locally first. If the restore
                fails part-way, they are put back automatically.
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
        runningRef.current = true;
        // pushedAt lets the backend refuse if the slot now holds a newer
        // snapshot than the one named in the dialog.
        const result = await bridge.restoreSnapshot(snap.index, snap.machineId, snap.pushedAt);
        runningRef.current = false;
        onStatusChange(await bridge.getStatus());
        setRestoreReport(result);
        if (result.warnings.length === 0) {
          setTimeout(() => setRestoreReport((r) => (r === result ? null : r)), 8000);
        }
      } catch (e) {
        setRestoreError(String(e));
      } finally {
        setRestoringKey(null);
        runningRef.current = false;
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
