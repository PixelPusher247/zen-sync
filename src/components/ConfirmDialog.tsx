import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useId,
  useRef,
  useState,
  type ReactNode,
} from "react";

export interface ConfirmOptions {
  title: string;
  message: ReactNode;
  confirmLabel?: string;
  /** Style the confirm button as destructive. */
  danger?: boolean;
  /** Focus Cancel first without the destructive styling, so Enter doesn't confirm. */
  cautious?: boolean;
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
    (options) =>
      new Promise((resolve) =>
        setPending((previous) => {
          // Only one dialog shows at a time. A dialog that gets replaced
          // counts as cancelled, so whoever was waiting on it carries on
          // instead of waiting forever.
          previous?.resolve(false);
          return { options, resolve };
        })
      ),
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
  cautious = false,
  onClose,
}: ConfirmOptions & { onClose: (ok: boolean) => void }) {
  const titleId = useId();
  const messageId = useId();
  const panelRef = useRef<HTMLDivElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const confirmRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    // Destructive actions shouldn't be one Enter press away.
    (danger || cautious ? cancelRef : confirmRef).current?.focus();
  }, [danger, cautious]);

  // On the document, not the overlay: clicking the message text moves focus
  // out of the buttons, and Escape and the focus trap must still work then.
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose(false);
      } else if (e.key === "Tab") {
        const buttons = [cancelRef.current, confirmRef.current];
        const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
        // Two buttons, so Tab and Shift+Tab both mean "the other one".
        const next = at === -1 ? buttons[0] : buttons[(at + 1) % 2];
        e.preventDefault();
        next?.focus();
      }
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-5 bg-black/60 animate-fade-in"
      onMouseDown={(e) => {
        if (!panelRef.current?.contains(e.target as Node)) onClose(false);
      }}
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
