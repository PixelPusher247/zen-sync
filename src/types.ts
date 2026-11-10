export interface SnapshotInfo {
  index: number;
  pushedAt: string;
  machineName: string;
  sizeMb: number;
  isCurrent: boolean;
  machineId: string;
}

export interface AppStatus {
  connected: boolean;
  username: string | null;
  machineName: string;
  /** Id this device's snapshots are stored under; matches SnapshotInfo.machineId. */
  machineId?: string;
  lastBackupAt: string | null;
  snapshotCount: number;
  autostartEnabled: boolean;
  syncOptions: SyncOptions;
  /** A saved sign-in is still being restored, e.g. while the network comes up. */
  reconnecting?: boolean;
  /**
   * The snapshot key on this device: "needs-setup" (no passphrase chosen
   * yet), "locked" (enter the passphrase or recovery code) or "ready".
   */
  encryption?: EncryptionState;
  /** With "needs-setup": setting up deletes backups from Zen Sync 0.5 and earlier. */
  oldBackups?: boolean;
}

export type EncryptionState = "unknown" | "needs-setup" | "locked" | "ready";

export interface SetupEncryptionResult {
  /** Shown once; the app never stores it. */
  recoveryCode: string;
  status: AppStatus;
  warning: string | null;
}

/** What this device backs up and restores. */
export interface SyncOptions {
  sineMods: boolean;
  modSettings: boolean;
  extensionStorage: boolean;
  extensionPermissions: boolean;
  extensionShortcuts: boolean;
  zenShortcuts: boolean;
  aboutConfig: boolean;
}

export interface UpdateInfo {
  version: string;
  notes: string;
  /** Portable builds open the download page instead of installing. */
  portable: boolean;
}

export interface ExtensionWithSelection {
  id: string;
  name: string;
  version: string;
  enabled: boolean;
  iconUrl: string | null;
  synced: boolean;
  /** Size of the extension's storage.local folder (0 if it has none). */
  storageBytes: number;
  passwordManager: boolean;
}

export interface BackupSummary {
  sineInstalled: boolean;
  sineEngineVersion: string | null;
  modCount: number;
  modSettingCount: number;
  extensionCount: number;
  storageBytes: number;
  shortcutCount: number;
  prefCount: number;
}

export interface RestoreReport {
  modCount: number;
  extensionCount: number;
  prefCount: number;
  shortcutsRestored: boolean;
  warnings: string[];
}

export interface PrefWithSelection {
  name: string;
  /** The raw prefs.js value literal, shown as written. */
  value: string;
  synced: boolean;
}

export type Screen =
  | "setup"
  | "dashboard"
  | "snapshots"
  | "settings"
  | "extensions"
  | "prefs"
  | "encryption";
