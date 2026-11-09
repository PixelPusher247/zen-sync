import type { SaveState } from "../hooks/useAutosave";
import OperationStatus from "./OperationStatus";
import { CloseButton } from "./WindowControls";

interface Props {
  title: string;
  subtitle: string;
  saveState: SaveState;
  onBack: () => void;
}

const SAVE_LABEL: Record<SaveState, string> = {
  idle: "",
  saving: "Saving…",
  saved: "✓ Saved",
  error: "Not saved",
};

/** Header of the full-screen pickers: back, title, autosave state, window close. */
export default function PickerHeader({ title, subtitle, saveState, onBack }: Props) {
  return (
    <>
    <div
      data-tauri-drag-region="deep"
      className="flex items-center gap-3 pl-3 pr-3 py-3 border-b border-surface-border bg-surface-raised shrink-0"
    >
      <button
        type="button"
        onClick={onBack}
        aria-label="Back to Settings"
        className="w-7 h-7 shrink-0 flex items-center justify-center rounded-lg text-subtle
                   hover:bg-surface-overlay hover:text-white transition-colors"
      >
        ←
      </button>
      <div className="flex-1 min-w-0">
        <h2 className="text-sm font-semibold text-white truncate">{title}</h2>
        <p className="text-xs text-muted truncate">{subtitle}</p>
      </div>
      <span
        role="status"
        className={`text-xs shrink-0 ${
          saveState === "error" ? "text-danger" : saveState === "saved" ? "text-success" : "text-muted"
        }`}
      >
        {SAVE_LABEL[saveState]}
      </span>
      <CloseButton />
    </div>
    {/* A backup or restore may still be running, or have just finished. */}
    <OperationStatus />
    </>
  );
}

/** "Select all · none" pair shown beside a picker's count line. */
export function SelectAllNone({
  label,
  allSelected,
  noneSelected,
  onAll,
  onNone,
}: {
  label: string;
  allSelected: boolean;
  noneSelected: boolean;
  onAll: () => void;
  onNone: () => void;
}) {
  return (
    <span className="flex items-center gap-1.5 text-xs shrink-0">
      <span className="text-muted">{label}</span>
      <button
        type="button"
        onClick={onAll}
        disabled={allSelected}
        className="text-accent hover:text-accent-hover disabled:opacity-40 transition-colors"
      >
        all
      </button>
      <span className="text-muted" aria-hidden="true">·</span>
      <button
        type="button"
        onClick={onNone}
        disabled={noneSelected}
        className="text-accent hover:text-accent-hover disabled:opacity-40 transition-colors"
      >
        none
      </button>
    </span>
  );
}
