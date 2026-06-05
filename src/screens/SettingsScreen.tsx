import { useState } from "react";
import { bridge } from "../bridge";
import type { AppStatus, Screen } from "../types";

interface Props {
  status: AppStatus;
  onStatusChange: (s: AppStatus) => void;
  onNavigate: (s: Screen) => void;
}

export default function SettingsScreen({ status, onStatusChange, onNavigate }: Props) {
  const [machineName, setMachineName] = useState(status.machineName);
  const [snapshotCount, setSnapshotCount] = useState(status.snapshotCount);
  const [autostart, setAutostart] = useState(status.autostartEnabled);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [disconnecting, setDisconnecting] = useState(false);

  async function handleSave() {
    setSaving(true);
    setError(null);
    try {
      await Promise.all([
        bridge.setMachineName(machineName),
        bridge.setSnapshotCount(snapshotCount),
        bridge.setAutostart(autostart),
      ]);
      const s = await bridge.getStatus();
      onStatusChange(s);
      setSaved(true);
      setTimeout(() => setSaved(false), 2500);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  async function handleDisconnect() {
    if (!confirm("Disconnect from GitHub? Your backup repo will not be deleted.")) {
      return;
    }
    setDisconnecting(true);
    try {
      await bridge.disconnectGithub();
      const s = await bridge.getStatus();
      onStatusChange(s);
    } catch (e) {
      setError(String(e));
      setDisconnecting(false);
    }
  }

  async function handleOpenLog() {
    try {
      await bridge.openLog();
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="p-5 flex flex-col gap-5 animate-fade-in">
      <div>
        <h2 className="text-sm font-semibold text-white">Settings</h2>
      </div>

      {/* Device */}
      <div className="card p-4 flex flex-col gap-3">
        <p className="section-label">This device</p>
        <div className="flex flex-col gap-1.5">
          <label className="text-xs text-muted" htmlFor="machine-name">
            Device name
          </label>
          <input
            id="machine-name"
            type="text"
            value={machineName}
            onChange={(e) => setMachineName(e.target.value)}
            placeholder="e.g. Work Laptop"
            className="input"
          />
          <p className="text-xs text-muted">
            Used to label your backups. Each device keeps its own name.
          </p>
        </div>
      </div>

      {/* Backup */}
      <div className="card p-4 flex flex-col gap-3">
        <p className="section-label">Backup</p>
        <div className="flex flex-col gap-1.5">
          <label className="text-xs text-muted" htmlFor="snapshot-count">
            Snapshots to keep per device
          </label>
          <div className="flex items-center gap-3">
            <input
              id="snapshot-count"
              type="range"
              min={1}
              max={10}
              value={snapshotCount}
              onChange={(e) => setSnapshotCount(Number(e.target.value))}
              className="flex-1 accent-accent"
            />
            <span className="w-6 text-center text-sm font-semibold text-white">
              {snapshotCount}
            </span>
          </div>
          <p className="text-xs text-muted">
            Older snapshots are automatically removed.
          </p>
        </div>
        <div className="divider" />
        <button
          type="button"
          onClick={() => onNavigate("extensions")}
          className="flex items-center justify-between w-full -mx-4 px-4 py-2 rounded-xl
                     hover:bg-surface-overlay transition-colors group"
        >
          <span className="text-sm text-white">Extensions to sync</span>
          <span className="text-muted text-xs group-hover:text-accent transition-colors">
            ›
          </span>
        </button>
      </div>

      {/* Startup */}
      <div className="card p-4">
        <p className="section-label">Startup</p>
        <div className="flex items-center justify-between mt-1">
          <div>
            <p className="text-sm text-white">Launch at login</p>
            <p className="text-xs text-muted mt-0.5">
              Start minimised to the system tray when you log in.
            </p>
          </div>
          <button
            type="button"
            role="switch"
            aria-checked={autostart}
            aria-label="Launch at login"
            onClick={() => setAutostart(!autostart)}
            className={`
              relative w-11 h-6 rounded-full transition-colors duration-200
              ${autostart ? "bg-accent" : "bg-surface-border"}
            `}
          >
            <span className={autostart ? "toggle-thumb-on" : "toggle-thumb-off"} />
          </button>
        </div>
      </div>

      {error && (
        <div className="p-3 bg-danger/10 border border-danger/20 rounded-xl text-sm text-danger animate-fade-in">
          {error}
        </div>
      )}

      {saved && (
        <div className="flex items-center gap-2 p-3 bg-success/10 border border-success/20 rounded-xl text-sm text-success animate-fade-in">
          <span>✓</span>
          <span>Settings saved</span>
        </div>
      )}

      <button
        type="button"
        onClick={handleSave}
        disabled={saving}
        className="btn-primary w-full"
      >
        {saving ? (
          <>
            <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
            Saving…
          </>
        ) : (
          "Save settings"
        )}
      </button>

      {/* Debug / account */}
      <div className="flex flex-col gap-2">
        <div className="divider" />
        <button
          type="button"
          onClick={handleOpenLog}
          className="btn-secondary text-xs w-full"
        >
          Open log file
        </button>
        <button
          type="button"
          onClick={handleDisconnect}
          disabled={disconnecting}
          className="btn-danger text-xs w-full"
        >
          {disconnecting ? "Disconnecting…" : "Disconnect GitHub"}
        </button>
      </div>
    </div>
  );
}
