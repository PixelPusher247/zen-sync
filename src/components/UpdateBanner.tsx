import { useState } from "react";
import { bridge } from "../bridge";
import type { UpdateInfo } from "../types";

interface Props {
  info: UpdateInfo;
  onDismiss: () => void;
}

export default function UpdateBanner({ info, onDismiss }: Props) {
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleInstall() {
    setInstalling(true);
    setError(null);
    try {
      await bridge.installUpdate();
      // Installed builds restart; portable builds only opened the download page.
      if (info.portable) setInstalling(false);
    } catch (e) {
      console.error("Install update failed:", e);
      setError(String(e));
      setInstalling(false);
    }
  }

  return (
    <div className="px-4 py-2.5 bg-accent/10 border-b border-accent/20 text-sm animate-slide-up">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-accent" aria-hidden="true">↑</span>
          <span className="text-white font-medium truncate">
            Zen Sync {info.version} is available
          </span>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <button
            type="button"
            onClick={handleInstall}
            disabled={installing}
            className="px-3 py-1 bg-accent text-white rounded-lg text-xs font-medium
                       hover:bg-accent-hover disabled:opacity-50 transition-colors"
          >
            {info.portable
              ? installing ? "Opening…" : "Download"
              : installing ? "Installing…" : "Install"}
          </button>
          <button
            type="button"
            aria-label="Dismiss update banner"
            onClick={onDismiss}
            className="text-muted hover:text-white transition-colors text-xs px-1"
          >
            ✕
          </button>
        </div>
      </div>
      {error && (
        <p role="alert" className="mt-1.5 text-xs text-danger select-text break-words">
          Update failed: {error}
        </p>
      )}
    </div>
  );
}
