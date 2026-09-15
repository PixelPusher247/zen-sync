import { useState, useEffect, useCallback } from "react";
import { bridge } from "../bridge";
import { formatBytes } from "../format";
import type { ExtensionWithSelection } from "../types";
import { CloseButton } from "../components/WindowControls";

interface Props {
  onBack: () => void;
}

export default function ExtensionsScreen({ onBack }: Props) {
  const [extensions, setExtensions] = useState<ExtensionWithSelection[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Local toggle state — key: extension id, value: synced
  const [toggles, setToggles] = useState<Record<string, boolean>>({});

  const load = useCallback(async () => {
    try {
      const list = await bridge.getExtensions();
      setExtensions(list);
      const initial: Record<string, boolean> = {};
      for (const ext of list) {
        initial[ext.id] = ext.synced;
      }
      setToggles(initial);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  function toggle(id: string) {
    setToggles((prev) => ({ ...prev, [id]: !prev[id] }));
  }

  function selectAll() {
    setToggles((prev) => Object.fromEntries(Object.keys(prev).map((k) => [k, true])));
  }

  function selectNone() {
    setToggles((prev) => Object.fromEntries(Object.keys(prev).map((k) => [k, false])));
  }

  const allSelected = extensions?.every((e) => toggles[e.id] ?? true) ?? false;
  const noneSelected = extensions?.every((e) => !(toggles[e.id] ?? true)) ?? false;

  async function handleSave() {
    setSaving(true);
    setError(null);
    try {
      const selectedIds = extensions
        ?.filter((e) => toggles[e.id] ?? true)
        .map((e) => e.id) ?? [];
      await bridge.setExtensionSelection(selectedIds);
      setSaved(true);
      setTimeout(() => setSaved(false), 2500);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="flex flex-col h-full animate-fade-in">
      {/* Header */}
      <div
        data-tauri-drag-region="deep"
        className="flex items-center gap-3 pl-5 pr-3 py-3.5 border-b border-surface-border bg-surface-raised"
      >
        <button
          type="button"
          onClick={onBack}
          className="text-muted hover:text-white transition-colors text-sm"
        >
          ← Back
        </button>
        <div className="flex-1">
          <h2 className="text-sm font-semibold text-white">Extensions</h2>
          <p className="text-xs text-muted">
            Choose which extensions' local data to include in backups
          </p>
        </div>
        {extensions && extensions.length > 0 && (
          <div className="flex items-center gap-1">
            <button
              type="button"
              onClick={selectAll}
              disabled={allSelected}
              className="text-xs text-accent hover:text-accent-hover disabled:opacity-40 transition-colors"
            >
              All
            </button>
            <span className="text-muted text-xs">/</span>
            <button
              type="button"
              onClick={selectNone}
              disabled={noneSelected}
              className="text-xs text-muted hover:text-white disabled:opacity-40 transition-colors"
            >
              None
            </button>
          </div>
        )}
        <CloseButton />
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4 flex flex-col gap-3">
        {loading && (
          <div className="flex items-center justify-center py-10">
            <div className="w-5 h-5 border-2 border-accent border-t-transparent rounded-full animate-spin" />
          </div>
        )}

        {error && (
          <div className="p-3 bg-danger/10 border border-danger/20 rounded-xl text-sm text-danger">
            {error}
          </div>
        )}

        {!loading && extensions?.length === 0 && (
          <div className="card p-6 text-center">
            <p className="text-muted text-sm">No extensions found.</p>
            <p className="text-muted text-xs mt-1">
              Install extensions in Zen Browser, then return here.
            </p>
          </div>
        )}

        {!loading && extensions && extensions.length > 0 && (
          <>
            <p className="text-xs text-muted">
              {Object.values(toggles).filter(Boolean).length} of{" "}
              {extensions.length} extensions will be backed up: their local
              storage, granted permissions and custom shortcuts. Password
              managers are off by default because their data includes your
              account session.
            </p>

            <div className="flex flex-col gap-1.5">
              {extensions.map((ext) => {
                const isOn = toggles[ext.id] ?? true;
                return (
                  <button
                    key={ext.id}
                    type="button"
                    className={`card px-3.5 py-2.5 flex items-center gap-3 w-full text-left
                      hover:border-surface-overlay transition-colors
                      ${isOn ? "" : "opacity-50"}`}
                    onClick={() => toggle(ext.id)}
                  >
                    {/* Icon */}
                    <div className="w-8 h-8 rounded-lg bg-surface-overlay flex items-center justify-center shrink-0 overflow-hidden">
                      {ext.iconUrl ? (
                        <img
                          src={ext.iconUrl}
                          alt=""
                          className="w-6 h-6 object-contain"
                          onError={(e) => {
                            (e.target as HTMLImageElement).style.display = "none";
                          }}
                        />
                      ) : (
                        <span className="text-muted text-xs font-bold">
                          {ext.name.charAt(0).toUpperCase()}
                        </span>
                      )}
                    </div>

                    {/* Info */}
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 min-w-0">
                        <p className="text-sm font-medium text-white truncate">
                          {ext.name}
                        </p>
                        {ext.passwordManager && (
                          <span className="badge-warning shrink-0">Password manager</span>
                        )}
                      </div>
                      <p className="text-xs text-muted truncate">
                        v{ext.version} ·{" "}
                        {ext.storageBytes > 0 ? formatBytes(ext.storageBytes) : "no local storage"}
                        {!ext.enabled && (
                          <span className="ml-2 opacity-60">(disabled)</span>
                        )}
                      </p>
                    </div>

                    {/* Toggle */}
                    <div
                      className={`w-9 h-5 rounded-full transition-colors duration-200 shrink-0
                        ${isOn ? "bg-accent" : "bg-surface-border"}`}
                    >
                      <span className={isOn ? "toggle-sm-thumb-on" : "toggle-sm-thumb-off"} />
                    </div>
                  </button>
                );
              })}
            </div>
          </>
        )}
      </div>

      {/* Footer */}
      {!loading && extensions && extensions.length > 0 && (
        <div className="p-4 border-t border-surface-border flex flex-col gap-2">
          {saved && (
            <div className="flex items-center gap-2 p-2.5 bg-success/10 border border-success/20 rounded-xl text-sm text-success animate-fade-in">
              <span>✓</span>
              <span>Extension selection saved</span>
            </div>
          )}
          {error && (
            <div className="p-2.5 bg-danger/10 border border-danger/20 rounded-xl text-sm text-danger">
              {error}
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
              "Save selection"
            )}
          </button>
          <p className="text-xs text-muted text-center">
            Changes take effect on the next backup.
          </p>
        </div>
      )}
    </div>
  );
}
