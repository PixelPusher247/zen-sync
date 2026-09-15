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
  lastBackupAt: string | null;
  snapshotCount: number;
  autostartEnabled: boolean;
}

export interface UpdateInfo {
  version: string;
  notes: string;
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
}

export interface RestoreReport {
  modCount: number;
  extensionCount: number;
  warnings: string[];
}

export type Screen = "setup" | "dashboard" | "snapshots" | "settings" | "extensions";
