import { getCurrentWindow } from "@tauri-apps/api/window";

// The main window has no native frame: headers double as drag regions
// ("deep" = the whole subtree drags, except buttons) and carry this button.

export function CloseButton() {
  return (
    <button
      type="button"
      aria-label="Close"
      title="Close"
      onClick={() =>
        getCurrentWindow()
          .close()
          .catch((e) => console.error("close failed:", e))
      }
      className="w-7 h-7 shrink-0 flex items-center justify-center rounded-lg text-xs text-muted
                 hover:bg-danger hover:text-white transition-colors"
    >
      ✕
    </button>
  );
}

/** Drag strip with a close button for screens without a header. Needs a `relative` parent. */
export function FloatingTitleBar() {
  return (
    <div
      data-tauri-drag-region="deep"
      className="absolute inset-x-0 top-0 h-12 flex items-center justify-end px-3"
    >
      <CloseButton />
    </div>
  );
}
