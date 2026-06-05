import { useState } from "react";
import { bridge } from "../bridge";
import type { UpdateInfo } from "../types";

interface Props {
  info: UpdateInfo;
  onDismiss: () => void;
}

export default function UpdateBanner({ info, onDismiss }: Props) {
  const [installing, setInstalling] = useState(false);

  async function handleInstall() {
    setInstalling(true);
    try {
      await bridge.installUpdate();
    } catch (e) {
      console.error("Install update failed:", e);
      setInstalling(false);
    }
  }

  return (
    <div className="flex items-center justify-between gap-3 px-4 py-2.5 bg-accent/10 border-b border-accent/20 text-sm animate-slide-up">
      <div className="flex items-center gap-2">
        <span className="text-accent">↑</span>
        <span className="text-white font-medium">
          Zen Sync {info.version} is available
        </span>
      </div>
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={handleInstall}
          disabled={installing}
          className="px-3 py-1 bg-accent text-white rounded-lg text-xs font-medium
                     hover:bg-accent-hover disabled:opacity-50 transition-colors"
        >
          {installing ? "Installing…" : "Install"}
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
  );
}
