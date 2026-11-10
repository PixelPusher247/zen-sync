import { useState, useEffect, useRef, type CSSProperties } from "react";
import { bridge } from "../bridge";
import { hasExtensionData } from "../format";
import type { AppStatus, Screen, SyncOptions } from "../types";
import { useConfirm } from "../components/ConfirmDialog";
import Switch from "../components/Switch";
import { allSaved, onBackgroundSaveFailure } from "../hooks/useAutosave";
import { useOperations } from "../hooks/useOperations";

interface Props {
  status: AppStatus;
  onStatusChange: (s: AppStatus) => void;
  onNavigate: (s: Screen) => void;
}

interface SyncItem {
  key: keyof SyncOptions;
  label: string;
  description: string;
}

const MOD_ITEMS: SyncItem[] = [
  { key: "sineMods", label: "Sine mods", description: "Installed mods and their files" },
  { key: "modSettings", label: "Mod settings", description: "Values you set for mods and Sine" },
];

const EXTENSION_ITEMS: SyncItem[] = [
  { key: "extensionStorage", label: "Extension data", description: "What extensions keep in local storage" },
  { key: "extensionPermissions", label: "Extension permissions", description: "Optional permissions you granted" },
  { key: "extensionShortcuts", label: "Extension shortcuts", description: "Custom keyboard shortcuts" },
];

const ZEN_ITEMS: SyncItem[] = [
  { key: "zenShortcuts", label: "Zen shortcuts", description: "Workspace, compact mode and other Zen keys" },
  { key: "aboutConfig", label: "about:config prefs", description: "Settings you changed in about:config" },
];

const MIN_SNAPSHOTS = 1;
const MAX_SNAPSHOTS = 10;

/** "6 of 7 selected", or nothing while the count is unknown. */
interface Count {
  selected: number;
  total: number;
}

/** Link to a picker, placed right under the switches it belongs to. */
function PickerLink({ label, count, onClick }: { label: string; count: Count | null; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex items-center justify-between gap-3 px-3 py-2 rounded-xl bg-surface-overlay
                 border border-surface-border hover:border-accent-muted transition-colors group text-left"
    >
      <span className="text-xs font-medium text-white">{label}</span>
      <span className="flex items-center gap-2 text-xs text-muted">
        {count && `${count.selected} of ${count.total} selected`}
        <span className="group-hover:text-accent transition-colors" aria-hidden="true">›</span>
      </span>
    </button>
  );
}

export default function SettingsScreen({ status, onStatusChange, onNavigate }: Props) {
  const confirm = useConfirm();
  const [syncOptions, setSyncOptions] = useState(status.syncOptions);
  const [machineName, setMachineName] = useState(status.machineName);
  const [snapshotCount, setSnapshotCount] = useState(status.snapshotCount);
  const [autostart, setAutostart] = useState(status.autostartEnabled);
  useEffect(() => { setAutostart(status.autostartEnabled); }, [status.autostartEnabled]);
  const [machineNameSaved, setMachineNameSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [disconnecting, setDisconnecting] = useState(false);
  const [version, setVersion] = useState<string | null>(null);
  const [extensionCount, setExtensionCount] = useState<Count | null>(null);
  const [prefCount, setPrefCount] = useState<Count | null>(null);
  const ops = useOperations();
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // The name waiting for the debounce, saved at once if the screen closes.
  const pendingNameRef = useRef<string | null>(null);
  const mountedRef = useRef(true);

  // Shown next to the picker links; the links work without them. A picker
  // may still be saving its last change, so wait for that first.
  useEffect(() => {
    let active = true;
    bridge.getVersion().then((v) => active && setVersion(v)).catch(() => {});
    allSaved().then(() => {
      if (!active) return;
      bridge
        .getExtensions()
        .then((list) => active && setExtensionCount({ selected: list.filter((e) => e.synced).length, total: list.length }))
        .catch(() => {});
      bridge
        .getPrefs()
        .then((list) => active && setPrefCount({ selected: list.filter((p) => p.synced).length, total: list.length }))
        .catch(() => {});
    });
    const unsubscribe = onBackgroundSaveFailure((message) =>
      setError(`Your last selection change wasn't saved: ${message}`),
    );
    return () => {
      active = false;
      unsubscribe();
    };
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      // Leaving within the debounce must not drop the new name.
      if (debounceRef.current) clearTimeout(debounceRef.current);
      if (pendingNameRef.current !== null) void saveMachineName(pendingNameRef.current);
    };
    // Runs on unmount only; saveMachineName reads nothing that goes stale.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const nameEmpty = machineName.trim() === "";

  async function saveMachineName(name: string) {
    pendingNameRef.current = null;
    try {
      await bridge.setMachineName(name);
      onStatusChange(await bridge.getStatus());
      if (!mountedRef.current) return;
      setMachineNameSaved(true);
      setTimeout(() => mountedRef.current && setMachineNameSaved(false), 2000);
    } catch (e) {
      if (mountedRef.current) setError(String(e));
    }
  }

  function handleMachineNameChange(value: string) {
    setMachineName(value);
    setMachineNameSaved(false);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    // Backups are filed under the name, so an empty one is never saved.
    if (value.trim() === "") {
      pendingNameRef.current = null;
      return;
    }
    pendingNameRef.current = value.trim();
    debounceRef.current = setTimeout(() => {
      debounceRef.current = null;
      if (pendingNameRef.current !== null) void saveMachineName(pendingNameRef.current);
    }, 600);
  }

  async function handleSnapshotCountChange(value: number) {
    const previous = snapshotCount;
    setSnapshotCount(value);
    try {
      await bridge.setSnapshotCount(value);
      const s = await bridge.getStatus();
      onStatusChange(s);
    } catch (e) {
      setSnapshotCount(previous);
      setError(String(e));
    }
  }

  async function handleSyncOptionChange(key: keyof SyncOptions, value: boolean) {
    const previous = syncOptions;
    const next = { ...previous, [key]: value };
    setSyncOptions(next);
    try {
      await bridge.setSyncOptions(next);
      onStatusChange(await bridge.getStatus());
    } catch (e) {
      setSyncOptions(previous);
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
    if (ops.busy) return;
    const ok = await confirm({
      title: "Disconnect from GitHub?",
      message:
        "Your backup repo and its snapshots are not deleted. You can reconnect at any time; you'll need your backup passphrase or recovery code again.",
      confirmLabel: "Disconnect",
      danger: true,
    });
    if (!ok) return;
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

  function renderSwitches(items: SyncItem[]) {
    return items.map((item) => (
      <Switch
        key={item.key}
        label={item.label}
        description={item.description}
        checked={syncOptions[item.key]}
        onChange={(value) => handleSyncOptionChange(item.key, value)}
      />
    ));
  }

  const sliderFill = ((snapshotCount - MIN_SNAPSHOTS) / (MAX_SNAPSHOTS - MIN_SNAPSHOTS)) * 100;

  return (
    <div className="p-5 flex flex-col gap-5 animate-fade-in">
      <div>
        <h2 className="text-sm font-semibold text-white">Settings</h2>
        <p className="text-xs text-muted mt-0.5">Changes are saved as you make them.</p>
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
              onBlur={() => {
                if (nameEmpty) setMachineName(status.machineName);
              }}
              placeholder="e.g. Work Laptop"
              aria-invalid={nameEmpty ? "true" : undefined}
              aria-describedby="machine-name-hint"
              className="input w-full pr-8"
            />
            {machineNameSaved && (
              <span className="absolute right-2.5 top-1/2 -translate-y-1/2 text-success text-sm animate-fade-in">
                ✓
              </span>
            )}
          </div>
          <p id="machine-name-hint" className={`text-xs ${nameEmpty ? "text-warning" : "text-muted"}`}>
            {nameEmpty
              ? `The name can't be empty. Still using “${status.machineName}”.`
              : "Used to label your backups. Each device keeps its own name."}
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
              min={MIN_SNAPSHOTS}
              max={MAX_SNAPSHOTS}
              value={snapshotCount}
              onChange={(e) => handleSnapshotCountChange(Number(e.target.value))}
              style={{ "--fill": `${sliderFill}%` } as CSSProperties}
              className="slider flex-1"
            />
            <span className="w-6 text-center text-sm font-semibold text-white">
              {snapshotCount}
            </span>
          </div>
          <p className="text-xs text-muted">
            Older snapshots are removed after each backup.
          </p>
        </div>
      </div>

      {/* What to sync */}
      <div className="card p-4 flex flex-col gap-3">
        <div>
          <p className="section-label mb-0.5">What to sync</p>
          <p className="text-xs text-muted">
            Applies to backups and restores on this device.
          </p>
        </div>

        {renderSwitches(MOD_ITEMS)}

        <div className="divider" />
        {renderSwitches(EXTENSION_ITEMS)}
        {hasExtensionData(syncOptions) && (
          <PickerLink
            label="Choose extensions"
            count={extensionCount}
            onClick={() => onNavigate("extensions")}
          />
        )}

        <div className="divider" />
        {renderSwitches(ZEN_ITEMS)}
        {syncOptions.aboutConfig && (
          <PickerLink
            label="Choose prefs"
            count={prefCount}
            onClick={() => onNavigate("prefs")}
          />
        )}
      </div>

      {/* Encryption */}
      <div className="card p-4 flex flex-col gap-3">
        <div>
          <p className="section-label mb-0.5">Encryption</p>
          <p className="text-xs text-muted">
            Backups are encrypted with a key only your passphrase or recovery
            code can unlock.
          </p>
        </div>
        <PickerLink
          label="Passphrase and recovery code"
          count={null}
          onClick={() => onNavigate("encryption")}
        />
      </div>

      {/* Startup */}
      <div className="card p-4">
        <p className="section-label">Startup</p>
        <div className="mt-1">
          <Switch
            label="Launch at login"
            description="Open Zen Sync when you sign in to Windows."
            checked={autostart}
            onChange={handleAutostartToggle}
          />
        </div>
      </div>

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
          disabled={disconnecting || ops.busy}
          title={ops.busy ? "Wait for the backup or restore to finish" : undefined}
          className="btn-danger text-xs w-full"
        >
          {disconnecting ? "Disconnecting…" : "Disconnect GitHub"}
        </button>
        {version && (
          <p className="text-xs text-muted text-center mt-1 select-text">Zen Sync {version}</p>
        )}
      </div>

      {/* Sticks to the bottom edge so it is seen wherever the page is scrolled. */}
      {error && (
        <div
          role="alert"
          className="sticky bottom-3 p-3 bg-surface-raised border border-danger/40 rounded-xl text-sm text-danger shadow-2xl animate-fade-in"
        >
          <p className="select-text break-words">{error}</p>
          <button
            type="button"
            onClick={() => setError(null)}
            className="mt-2 text-xs underline opacity-80 hover:opacity-100"
          >
            Dismiss
          </button>
        </div>
      )}
    </div>
  );
}
