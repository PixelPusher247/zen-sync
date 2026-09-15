import type { Screen } from "../types";
import { CloseButton } from "./WindowControls";

interface Props {
  screen: Screen;
  onNavigate: (s: Screen) => void;
}

const NAV_ITEMS: { id: Screen; label: string; icon: string }[] = [
  { id: "dashboard", label: "Sync", icon: "⟳" },
  { id: "snapshots", label: "History", icon: "⊞" },
  { id: "settings", label: "Settings", icon: "⚙" },
];

export default function NavBar({ screen, onNavigate }: Props) {
  return (
    <header
      data-tauri-drag-region="deep"
      className="flex items-center justify-between pl-5 pr-3 py-3 border-b border-surface-border bg-surface-raised"
    >
      <div className="flex items-center gap-2.5">
        <div className="w-6 h-6 rounded-lg bg-accent/20 flex items-center justify-center">
          <span className="text-accent text-xs font-bold">Z</span>
        </div>
        <span className="text-sm font-semibold text-white tracking-tight">
          Zen Sync
        </span>
      </div>
      <div className="flex items-center gap-1">
        <nav className="flex items-center gap-1">
          {NAV_ITEMS.map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={() => onNavigate(item.id)}
              className={`
                px-3 py-1.5 rounded-lg text-xs font-medium transition-all duration-150
                ${
                  screen === item.id
                    ? "bg-accent/15 text-accent"
                    : "text-muted hover:text-white hover:bg-surface-overlay"
                }
              `}
            >
              {item.label}
            </button>
          ))}
        </nav>
        <div className="w-px h-4 mx-1 bg-surface-border" />
        <CloseButton />
      </div>
    </header>
  );
}
