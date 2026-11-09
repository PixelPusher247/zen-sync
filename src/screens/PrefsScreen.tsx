import { useState, useEffect, useCallback, useMemo } from "react";
import { bridge } from "../bridge";
import type { PrefWithSelection } from "../types";
import PickerHeader, { SelectAllNone } from "../components/PickerHeader";
import { useAutosave } from "../hooks/useAutosave";

interface Props {
  onBack: () => void;
}

/** Trim a raw prefs.js literal down to something that fits one line. */
function displayValue(raw: string): string {
  const unquoted = raw.length > 1 && raw.startsWith('"') && raw.endsWith('"')
    ? raw.slice(1, -1)
    : raw;
  return unquoted.length > 90 ? `${unquoted.slice(0, 90)}…` : unquoted;
}

export default function PrefsScreen({ onBack }: Props) {
  const [prefs, setPrefs] = useState<PrefWithSelection[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  // Names of the prefs that travel between devices.
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const autosave = useAutosave((names: string[]) => bridge.setPrefSelection(names));

  const load = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      const list = await bridge.getPrefs();
      setPrefs(list);
      setSelected(new Set(list.filter((p) => p.synced).map((p) => p.name)));
    } catch (e) {
      setLoadError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const visible = useMemo(() => {
    if (!prefs) return [];
    const needle = filter.trim().toLowerCase();
    if (!needle) return prefs;
    return prefs.filter(
      (p) =>
        p.name.toLowerCase().includes(needle) ||
        p.value.toLowerCase().includes(needle),
    );
  }, [prefs, filter]);

  /** Every change is saved straight away, like the switches in Settings. */
  function apply(next: Set<string>) {
    setSelected(next);
    autosave.push([...next]);
  }

  function toggle(name: string) {
    const next = new Set(selected);
    if (!next.delete(name)) next.add(name);
    apply(next);
  }

  /** All / none act on what the filter is showing. */
  function setVisible(on: boolean) {
    const next = new Set(selected);
    for (const p of visible) {
      if (on) next.add(p.name);
      else next.delete(p.name);
    }
    apply(next);
  }

  const count = prefs?.length ?? 0;
  const filtered = visible.length !== count;
  const allSelected = visible.length > 0 && visible.every((p) => selected.has(p.name));
  const noneSelected = visible.every((p) => !selected.has(p.name));

  return (
    <div className="flex flex-col h-full animate-fade-in">
      <PickerHeader
        title="about:config"
        subtitle="Prefs that travel between your devices"
        saveState={autosave.state}
        onBack={onBack}
      />

      <div className="flex-1 min-h-0 overflow-y-auto p-4 flex flex-col gap-3">
        {loading && (
          <div className="flex items-center justify-center py-10" role="status" aria-label="Loading prefs">
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
            <p className="text-subtle text-sm">Nothing to sync yet.</p>
            <p className="text-muted text-xs mt-1">
              Prefs you change in about:config show up here. Mod settings,
              extension state and anything tied to this machine are left out.
            </p>
          </div>
        )}

        {!loading && prefs && count > 0 && (
          <>
            <p className="text-xs text-muted">
              Selected prefs are backed up and applied on restore. Prefs
              describing this machine — hardware, sessions, profile paths, your
              Mozilla account — are never offered.
            </p>

            <input
              type="search"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              placeholder="Filter by name or value…"
              className="input w-full"
              aria-label="Filter prefs"
            />

            <div className="flex items-center justify-between gap-3">
              <p className="text-xs text-subtle font-medium">
                {selected.size} of {count} selected
                {filtered && <span className="text-muted font-normal"> · {visible.length} shown</span>}
              </p>
              {visible.length > 0 && (
                <SelectAllNone
                  label={filtered ? "Select shown" : "Select"}
                  allSelected={allSelected}
                  noneSelected={noneSelected}
                  onAll={() => setVisible(true)}
                  onNone={() => setVisible(false)}
                />
              )}
            </div>

            {visible.length > 0 ? (
              <div className="card overflow-hidden divide-y divide-surface-border">
                {visible.map((pref) => {
                  const isOn = selected.has(pref.name);
                  return (
                    <button
                      key={pref.name}
                      type="button"
                      role="switch"
                      aria-checked={isOn ? "true" : "false"}
                      aria-label={pref.name}
                      title={`${pref.name} = ${pref.value}`}
                      className="px-3 py-1.5 flex items-center gap-3 w-full text-left
                        hover:bg-surface-overlay transition-colors -outline-offset-2"
                      onClick={() => toggle(pref.name)}
                    >
                      <div className="flex-1 min-w-0">
                        <p className={`text-xs font-medium truncate ${isOn ? "text-white" : "text-subtle"}`}>
                          {pref.name}
                        </p>
                        <p className="text-[11px] text-muted truncate font-mono">
                          {displayValue(pref.value)}
                        </p>
                      </div>
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
            ) : (
              <p className="text-xs text-muted text-center py-6">
                No pref matches “{filter}”.
              </p>
            )}
          </>
        )}
      </div>
    </div>
  );
}
