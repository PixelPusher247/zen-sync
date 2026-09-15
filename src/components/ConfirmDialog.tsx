import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useId,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from "react";

export interface ConfirmOptions {
  title: string;
  message: ReactNode;
  confirmLabel?: string;
  /** Style the confirm button as destructive. */
  danger?: boolean;
}

type Confirm = (options: ConfirmOptions) => Promise<boolean>;

const ConfirmContext = createContext<Confirm | null>(null);

/**
 * In-window replacement for `window.confirm`, whose native dialog can end up
 * outside the small app window.
 */
export function useConfirm(): Confirm {
  const confirm = useContext(ConfirmContext);
  if (!confirm) throw new Error("useConfirm must be used inside ConfirmProvider");
  return confirm;
}

interface Pending {
  options: ConfirmOptions;
  resolve: (ok: boolean) => void;
}

export function ConfirmProvider({ children }: { children: ReactNode }) {
  const [pending, setPending] = useState<Pending | null>(null);

  const confirm = useCallback<Confirm>(
    (options) => new Promise((resolve) => setPending({ options, resolve })),
    []
  );

  const close = useCallback(
    (ok: boolean) => {
      pending?.resolve(ok);
      setPending(null);
    },
    [pending]
  );

  return (
    <ConfirmContext.Provider value={confirm}>
      {children}
      {pending && <ConfirmDialog {...pending.options} onClose={close} />}
    </ConfirmContext.Provider>
  );
}

function ConfirmDialog({
  title,
  message,
  confirmLabel = "Continue",
  danger = false,
  onClose,
}: ConfirmOptions & { onClose: (ok: boolean) => void }) {
  const titleId = useId();
  const messageId = useId();
  const panelRef = useRef<HTMLDivElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const confirmRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    // Destructive actions shouldn't be one Enter press away.
    (danger ? cancelRef : confirmRef).current?.focus();
  }, [danger]);

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onClose(false);
    } else if (e.key === "Tab") {
      // Keep focus inside the dialog.
      const buttons = [cancelRef.current, confirmRef.current];
      const next = buttons[(buttons.indexOf(document.activeElement as HTMLButtonElement) + 1) % 2];
      e.preventDefault();
      next?.focus();
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-5 bg-black/60 animate-fade-in"
      onMouseDown={(e) => {
        if (!panelRef.current?.contains(e.target as Node)) onClose(false);
      }}
      onKeyDown={handleKeyDown}
    >
      <div
        ref={panelRef}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={messageId}
        className="card w-full p-5 flex flex-col gap-4 shadow-2xl animate-slide-up"
      >
        <div className="flex flex-col gap-1.5">
          <h2 id={titleId} className="text-sm font-semibold text-white">
            {title}
          </h2>
          <div
            id={messageId}
            className="text-xs text-subtle leading-relaxed max-h-[50vh] overflow-y-auto"
          >
            {message}
          </div>
        </div>
        <div className="flex justify-end gap-2">
          <button
            ref={cancelRef}
            type="button"
            onClick={() => onClose(false)}
            className="btn-secondary text-xs"
          >
            Cancel
          </button>
          <button
            ref={confirmRef}
            type="button"
            onClick={() => onClose(true)}
            className={`${danger ? "btn-danger" : "btn-primary"} text-xs`}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
