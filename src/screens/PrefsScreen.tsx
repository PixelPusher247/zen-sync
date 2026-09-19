import { useState, useEffect, useCallback, useMemo } from "react";
import { bridge } from "../bridge";
import type { PrefWithSelection } from "../types";
import { CloseButton } from "../components/WindowControls";

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
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  // Local toggle state — key: pref name, value: synced
  const [toggles, setToggles] = useState<Record<string, boolean>>({});

  const load = useCallback(async () => {
    try {
      const list = await bridge.getPrefs();
      setPrefs(list);
      setToggles(Object.fromEntries(list.map((p) => [p.name, p.synced])));
    } catch (e) {
      setError(String(e));
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

  function toggle(name: string) {
    setToggles((prev) => ({ ...prev, [name]: !prev[name] }));
  }

  /** All / None act on what the filter is showing. */
  function setVisible(on: boolean) {
    setToggles((prev) => {
      const next = { ...prev };
      for (const p of visible) next[p.name] = on;
      return next;
    });
  }

  const selectedCount = Object.values(toggles).filter(Boolean).length;
  const allSelected = visible.length > 0 && visible.every((p) => toggles[p.name]);
  const noneSelected = visible.every((p) => !toggles[p.name]);

  async function handleSave() {
    setSaving(true);
    setError(null);
    try {
      await bridge.setPrefSelection(
        prefs?.filter((p) => toggles[p.name]).map((p) => p.name) ?? [],
      );
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
          <h2 className="text-sm font-semibold text-white">about:config</h2>
          <p className="text-xs text-muted">
            Choose which prefs travel between your devices
          </p>
        </div>
        {prefs && prefs.length > 0 && (
          <div className="flex items-center gap-1">
            <button
              type="button"
              onClick={() => setVisible(true)}
              disabled={allSelected}
              className="text-xs text-accent hover:text-accent-hover disabled:opacity-40 transition-colors"
            >
              All
            </button>
            <span className="text-muted text-xs">/</span>
            <button
              type="button"
              onClick={() => setVisible(false)}
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

        {!loading && prefs?.length === 0 && (
          <div className="card p-6 text-center">
            <p className="text-muted text-sm">Nothing to sync yet.</p>
            <p className="text-muted text-xs mt-1">
              Prefs you change in about:config show up here. Mod settings,
              extension state and anything tied to this machine are left out.
            </p>
          </div>
        )}

        {!loading && prefs && prefs.length > 0 && (
          <>
            <p className="text-xs text-muted">
              {selectedCount} of {prefs.length} prefs will be backed up and
              applied on restore. Prefs describing this machine — hardware,
              sessions, profile paths, your Mozilla account — are never offered,
              and prefs missing from a snapshot are left as they are here.
            </p>

            <input
              type="search"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              placeholder="Filter prefs…"
              className="input w-full"
              aria-label="Filter prefs"
            />

            <div className="flex flex-col gap-1.5">
              {visible.map((pref) => {
                const isOn = toggles[pref.name] ?? true;
                return (
                  <button
                    key={pref.name}
                    type="button"
                    className={`card px-3.5 py-2.5 flex items-center gap-3 w-full text-left
                      hover:border-surface-overlay transition-colors
                      ${isOn ? "" : "opacity-50"}`}
                    onClick={() => toggle(pref.name)}
                  >
                    <div className="flex-1 min-w-0">
                      <p className="text-sm font-medium text-white truncate">
                        {pref.name}
                      </p>
                      <p className="text-xs text-muted truncate font-mono">
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
              {visible.length === 0 && (
                <p className="text-xs text-muted text-center py-6">
                  No pref matches “{filter}”.
                </p>
              )}
            </div>
          </>
        )}
      </div>

      {/* Footer */}
      {!loading && prefs && prefs.length > 0 && (
        <div className="p-4 border-t border-surface-border flex flex-col gap-2">
          {saved && (
            <div className="flex items-center gap-2 p-2.5 bg-success/10 border border-success/20 rounded-xl text-sm text-success animate-fade-in">
              <span>✓</span>
              <span>Pref selection saved</span>
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
