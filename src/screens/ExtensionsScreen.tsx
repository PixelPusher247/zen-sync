import { useState, useEffect, useCallback } from "react";
import { bridge } from "../bridge";
import { formatBytes } from "../format";
import type { ExtensionWithSelection } from "../types";
import PickerHeader, { SelectAllNone } from "../components/PickerHeader";
import { useAutosave } from "../hooks/useAutosave";

interface Props {
  onBack: () => void;
}

export default function ExtensionsScreen({ onBack }: Props) {
  const [extensions, setExtensions] = useState<ExtensionWithSelection[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  // Ids of the extensions whose data is backed up.
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const autosave = useAutosave((ids: string[]) => bridge.setExtensionSelection(ids));

  const load = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      const list = await bridge.getExtensions();
      setExtensions(list);
      setSelected(new Set(list.filter((e) => e.synced).map((e) => e.id)));
    } catch (e) {
      setLoadError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  /** Every change is saved straight away, like the switches in Settings. */
  function apply(next: Set<string>) {
    setSelected(next);
    autosave.push([...next]);
  }

  function toggle(id: string) {
    const next = new Set(selected);
    if (!next.delete(id)) next.add(id);
    apply(next);
  }

  const count = extensions?.length ?? 0;

  return (
    <div className="flex flex-col h-full animate-fade-in">
      <PickerHeader
        title="Extensions"
        subtitle="Whose data to back up and restore"
        saveState={autosave.state}
        onBack={onBack}
      />

      <div className="flex-1 min-h-0 overflow-y-auto p-4 flex flex-col gap-3">
        {loading && (
          <div className="flex items-center justify-center py-10" role="status" aria-label="Loading extensions">
            <div className="w-5 h-5 border-2 border-accent border-t-transparent rounded-full animate-spin" />
          </div>
        )}

        {loadError && (
          <div role="alert" className="p-3 bg-danger/10 border border-danger/20 rounded-xl text-sm text-danger">
            <p className="select-text break-words">{loadError}</p>
            <button type="button" onClick={load} className="mt-2 text-xs underline opacity-80 hover:opacity-100">
              Try again
            </button>
          </div>
        )}

        {autosave.error && (
          <div role="alert" className="p-3 bg-danger/10 border border-danger/20 rounded-xl text-sm text-danger">
            <p className="font-medium">Couldn't save your selection</p>
            <p className="text-xs mt-1 select-text break-words">{autosave.error}</p>
            <button
              type="button"
              onClick={() => autosave.push([...selected])}
              className="mt-2 text-xs underline opacity-80 hover:opacity-100"
            >
              Try again
            </button>
          </div>
        )}

        {!loading && !loadError && count === 0 && (
          <div className="card p-6 text-center">
            <p className="text-subtle text-sm">No extensions found.</p>
            <p className="text-muted text-xs mt-1">
              Install extensions in Zen Browser, then return here.
            </p>
          </div>
        )}

        {!loading && extensions && count > 0 && (
          <>
            <p className="text-xs text-muted">
              Covers each extension's local storage, granted permissions and
              custom shortcuts. Password managers are off by default because
              their data includes your account session.
            </p>

            <div className="flex items-center justify-between gap-3">
              <p className="text-xs text-subtle font-medium">
                {selected.size} of {count} selected
              </p>
              <SelectAllNone
                label="Select"
                allSelected={selected.size === count}
                noneSelected={selected.size === 0}
                onAll={() => apply(new Set(extensions.map((e) => e.id)))}
                onNone={() => apply(new Set())}
              />
            </div>

            <div className="flex flex-col gap-1.5">
              {extensions.map((ext) => {
                const isOn = selected.has(ext.id);
                return (
                  <button
                    key={ext.id}
                    type="button"
                    role="switch"
                    aria-checked={isOn ? "true" : "false"}
                    title={ext.name}
                    className="card px-3.5 py-2.5 flex items-center gap-3 w-full text-left
                      hover:border-accent-muted transition-colors"
                    onClick={() => toggle(ext.id)}
                  >
                    {/* Icon */}
                    <div
                      className={`w-8 h-8 rounded-lg bg-surface-overlay flex items-center justify-center shrink-0 overflow-hidden
                        ${isOn ? "" : "opacity-50 grayscale"}`}
                    >
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
                        <p className={`text-sm font-medium truncate ${isOn ? "text-white" : "text-subtle"}`}>
                          {ext.name}
                        </p>
                        {ext.passwordManager && (
                          <span className="badge-warning shrink-0">Password manager</span>
                        )}
                      </div>
                      <p className="text-xs text-muted truncate">
                        v{ext.version} ·{" "}
                        {ext.storageBytes > 0 ? formatBytes(ext.storageBytes) : "no local storage"}
                        {!ext.enabled && " · disabled in Zen"}
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
    </div>
  );
}
