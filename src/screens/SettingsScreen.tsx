import { useState, useEffect, useRef } from "react";
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
  useEffect(() => { setAutostart(status.autostartEnabled); }, [status.autostartEnabled]);
  const [machineNameSaved, setMachineNameSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [disconnecting, setDisconnecting] = useState(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  function handleMachineNameChange(value: string) {
    setMachineName(value);
    setMachineNameSaved(false);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(async () => {
      try {
        await bridge.setMachineName(value);
        const s = await bridge.getStatus();
        onStatusChange(s);
        setMachineNameSaved(true);
        setTimeout(() => setMachineNameSaved(false), 2000);
      } catch (e) {
        setError(String(e));
      }
    }, 600);
  }

  async function handleSnapshotCountChange(value: number) {
    setSnapshotCount(value);
    try {
      await bridge.setSnapshotCount(value);
      const s = await bridge.getStatus();
      onStatusChange(s);
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleAutostartToggle() {
    const next = !autostart;
    setAutostart(next);
    try {
      await bridge.setAutostart(next);
      const s = await bridge.getStatus();
      onStatusChange(s);
    } catch (e) {
      setAutostart(!next);
      setError(String(e));
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
          <div className="relative">
            <input
              id="machine-name"
              type="text"
              value={machineName}
              onChange={(e) => handleMachineNameChange(e.target.value)}
              placeholder="e.g. Work Laptop"
              className="input w-full pr-8"
            />
            {machineNameSaved && (
              <span className="absolute right-2.5 top-1/2 -translate-y-1/2 text-success text-sm animate-fade-in">
                ✓
              </span>
            )}
          </div>
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
              onChange={(e) => handleSnapshotCountChange(Number(e.target.value))}
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
          <span className="text-sm text-white">Extensions to back up</span>
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
              Start minimized to the system tray when you log in.
            </p>
          </div>
          <button
            type="button"
            role="switch"
            aria-checked={autostart ? "true" : "false"}
            aria-label="Launch at login"
            onClick={handleAutostartToggle}
            className={`
              relative w-11 h-6 rounded-full transition-colors duration-200 appearance-none outline-none
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

      {/* Debug / account */}
      <div className="flex flex-col gap-2">
        <div className="divider" />
        <button
          type="button"
          onClick={handleOpenLog}
          className="btn-secondary text-xs w-full"
        >
          Show logs
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
