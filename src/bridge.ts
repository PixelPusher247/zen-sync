import { invoke } from "@tauri-apps/api/core";
import type {
  AppStatus,
  BackupSummary,
  ExtensionWithSelection,
  RestoreReport,
  SnapshotInfo,
} from "./types";

export const bridge = {
  getStatus: () => invoke<AppStatus>("get_status_cmd"),

  connectGithub: () => invoke<AppStatus>("connect_github_cmd"),

  disconnectGithub: () => invoke<void>("disconnect_github_cmd"),

  // Progress updates arrive via the "sync-progress" event — listen separately.
  backupNow: (deleteLegacy: boolean) =>
    invoke<void>("backup_now_cmd", { deleteLegacy }),

  getLegacySnapshotCount: () => invoke<number>("get_legacy_snapshot_count_cmd"),

  getBackupSummary: () => invoke<BackupSummary>("get_backup_summary_cmd"),

  getSnapshots: () => invoke<SnapshotInfo[]>("get_snapshots_cmd"),

  restoreSnapshot: (index: number, machineId: string) =>
    invoke<RestoreReport>("restore_snapshot_cmd", { index, machineId }),

  setMachineName: (name: string) =>
    invoke<void>("set_machine_name_cmd", { name }),

  setSnapshotCount: (count: number) =>
    invoke<void>("set_snapshot_count_cmd", { count }),

  setAutostart: (enabled: boolean) =>
    invoke<void>("set_autostart_cmd", { enabled }),

  getExtensions: () =>
    invoke<ExtensionWithSelection[]>("get_extensions_with_selection_cmd"),

  setExtensionSelection: (ids: string[]) =>
    invoke<void>("set_extension_selection_cmd", { ids }),

  isZenRunning: () => invoke<boolean>("is_zen_running"),

  installUpdate: () => invoke<void>("install_update"),

  openLog: () => invoke<void>("open_log_cmd"),

  getLog: () => invoke<string>("get_log_cmd"),
};
