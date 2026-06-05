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
}

export type Screen = "setup" | "dashboard" | "snapshots" | "settings" | "extensions";
