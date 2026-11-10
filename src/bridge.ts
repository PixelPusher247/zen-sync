import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import type {
  AppStatus,
  BackupSummary,
  ExtensionWithSelection,
  PrefWithSelection,
  RestoreReport,
  SetupEncryptionResult,
  SnapshotInfo,
  SyncOptions,
} from "./types";

export const bridge = {
  getStatus: () => invoke<AppStatus>("get_status_cmd"),

  connectGithub: () => invoke<AppStatus>("connect_github_cmd"),

  disconnectGithub: () => invoke<void>("disconnect_github_cmd"),

  /** First device: choose the passphrase. Returns the recovery code to show once. */
  setupEncryption: (passphrase: string) =>
    invoke<SetupEncryptionResult>("setup_encryption_cmd", { passphrase }),

  /** Another device: unlock with the passphrase or the recovery code. */
  unlockEncryption: (secret: string) =>
    invoke<AppStatus>("unlock_encryption_cmd", { secret }),

  changePassphrase: (current: string, newPassphrase: string) =>
    invoke<void>("change_passphrase_cmd", { current, newPassphrase }),

  /** Returns the new recovery code; the old one stops working. */
  newRecoveryCode: (passphrase: string) =>
    invoke<string>("new_recovery_code_cmd", { passphrase }),

  /** Deletes every backup and the key file. */
  resetEncryption: () => invoke<AppStatus>("reset_encryption_cmd"),

  // Progress updates arrive via the "sync-progress" event — listen separately.
  backupNow: (deleteLegacy: boolean) =>
    invoke<void>("backup_now_cmd", { deleteLegacy }),

  getLegacySnapshotCount: () => invoke<number>("get_legacy_snapshot_count_cmd"),

  getBackupSummary: () => invoke<BackupSummary>("get_backup_summary_cmd"),

  /** All devices' snapshots, newest first. */
  getSnapshots: () => invoke<SnapshotInfo[]>("get_snapshots_cmd"),

  /** `pushedAt` is the snapshot's time as listed; the backend refuses if the slot now holds another. */
  restoreSnapshot: (index: number, machineId: string, pushedAt: string) =>
    invoke<RestoreReport>("restore_snapshot_cmd", { index, machineId, pushedAt }),

  setMachineName: (name: string) =>
    invoke<void>("set_machine_name_cmd", { name }),

  setSnapshotCount: (count: number) =>
    invoke<void>("set_snapshot_count_cmd", { count }),

  setSyncOptions: (options: SyncOptions) =>
    invoke<void>("set_sync_options_cmd", { options }),

  setAutostart: (enabled: boolean) =>
    invoke<void>("set_autostart_cmd", { enabled }),

  getExtensions: () =>
    invoke<ExtensionWithSelection[]>("get_extensions_with_selection_cmd"),

  setExtensionSelection: (ids: string[]) =>
    invoke<void>("set_extension_selection_cmd", { ids }),

  /** Every about:config pref this profile could sync, with its choice. */
  getPrefs: () => invoke<PrefWithSelection[]>("get_prefs_with_selection_cmd"),

  setPrefSelection: (names: string[]) =>
    invoke<void>("set_pref_selection_cmd", { names }),

  isZenRunning: () => invoke<boolean>("is_zen_running"),

  installUpdate: () => invoke<void>("install_update"),

  openLog: () => invoke<void>("open_log_cmd"),

  getLog: () => invoke<string>("get_log_cmd"),

  getVersion: () => getVersion(),
};
