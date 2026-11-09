import type { Screen } from "../types";
import { useOperations } from "../hooks/useOperations";
import { CloseButton } from "./WindowControls";

interface Props {
  screen: Screen;
  onNavigate: (s: Screen) => void;
}

const NAV_ITEMS: { id: Screen; label: string }[] = [
  { id: "dashboard", label: "Sync" },
  { id: "snapshots", label: "History" },
  { id: "settings", label: "Settings" },
];

export default function NavBar({ screen, onNavigate }: Props) {
  const { busy } = useOperations();
  return (
    <header
      data-tauri-drag-region="deep"
      className="flex items-center justify-between pl-5 pr-3 py-3 border-b border-surface-border bg-surface-raised"
    >
      <div className="flex items-center gap-2.5">
        <img src="/app-icon.svg" alt="" className="w-6 h-6 rounded-lg" draggable={false} />
        <span className="text-sm font-semibold text-white tracking-tight">
          Zen Sync
        </span>
        {busy && (
          <div
            aria-hidden="true"
            className="w-3 h-3 border-2 border-accent/30 border-t-accent rounded-full animate-spin"
          />
        )}
      </div>
      <div className="flex items-center gap-1">
        <nav className="flex items-center gap-1">
          {NAV_ITEMS.map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={() => onNavigate(item.id)}
              aria-current={screen === item.id ? "page" : undefined}
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
