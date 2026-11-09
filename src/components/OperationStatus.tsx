import type { ReactNode } from "react";
import { bridge } from "../bridge";
import { useOperations } from "../hooks/useOperations";
import type { RestoreReport } from "../types";

function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}

/** "14 mods, 6 extensions and Zen shortcuts" for what a restore wrote. */
function describeRestored(report: RestoreReport): string {
  const parts = [
    report.modCount > 0 && plural(report.modCount, "mod"),
    report.extensionCount > 0 && plural(report.extensionCount, "extension"),
    report.shortcutsRestored && "Zen shortcuts",
    report.prefCount > 0 && plural(report.prefCount, "pref"),
  ].filter((part): part is string => Boolean(part));
  if (parts.length === 0) return "Restore complete.";
  const list =
    parts.length === 1
      ? parts[0]
      : `${parts.slice(0, -1).join(", ")} and ${parts[parts.length - 1]}`;
  return `Restored ${list}.`;
}

function LinkButton({ onClick, children }: { onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="text-xs underline opacity-80 hover:opacity-100"
    >
      {children}
    </button>
  );
}

function ErrorCard({
  title,
  message,
  onDismiss,
}: {
  title: string;
  message: string;
  onDismiss: () => void;
}) {
  return (
    <div
      role="alert"
      className="p-3 bg-danger/10 border border-danger/20 rounded-xl text-sm text-danger animate-fade-in"
    >
      <p className="font-medium">{title}</p>
      <p className="text-xs mt-1 opacity-90 whitespace-pre-line select-text break-words max-h-24 overflow-y-auto">
        {message}
      </p>
      <div className="mt-2 flex gap-4">
        <LinkButton onClick={() => bridge.openLog().catch(() => {})}>Show logs</LinkButton>
        <LinkButton onClick={onDismiss}>Dismiss</LinkButton>
      </div>
    </div>
  );
}

/**
 * Progress and outcome of the running or last backup/restore. Sits above the
 * scrolling content so it is visible on every screen and at any scroll position.
 */
export default function OperationStatus() {
  const ops = useOperations();
  const { restoreReport } = ops;

  const items: ReactNode[] = [];

  if (ops.busy) {
    items.push(
      <div
        key="progress"
        role="status"
        className="flex items-center gap-2.5 p-3 bg-accent/10 border border-accent/20 rounded-xl text-sm text-white"
      >
        <div className="w-4 h-4 shrink-0 border-2 border-accent/30 border-t-accent rounded-full animate-spin" />
        <span className="font-medium shrink-0">
          {ops.restoringKey ? "Restoring" : "Backing up"}
        </span>
        <span className="text-xs text-subtle truncate">{ops.progress}</span>
      </div>,
    );
  }

  if (ops.backupState === "success") {
    items.push(
      <div
        key="backup-ok"
        role="status"
        className="flex items-center gap-2 p-3 bg-success/10 border border-success/20 rounded-xl text-sm text-success animate-fade-in"
      >
        <span aria-hidden="true">✓</span>
        <span>Backup complete</span>
      </div>,
    );
  }

  if (ops.backupState === "error" && ops.backupError) {
    items.push(
      <ErrorCard
        key="backup-error"
        title="Backup failed"
        message={ops.backupError}
        onDismiss={ops.dismissBackup}
      />,
    );
  }

  if (restoreReport) {
    items.push(
      <div
        key="restore-ok"
        role="status"
        className="p-3 bg-success/10 border border-success/20 rounded-xl text-sm text-success animate-fade-in"
      >
        <div className="flex items-start gap-2">
          <span aria-hidden="true">✓</span>
          <div className="flex-1 min-w-0">
            <p>{describeRestored(restoreReport)}</p>
            <p className="text-xs opacity-80 mt-0.5">You can open Zen Browser again.</p>
          </div>
          <LinkButton onClick={ops.dismissRestore}>Dismiss</LinkButton>
        </div>
        {restoreReport.warnings.length > 0 && (
          <ul className="mt-2 pt-2 border-t border-success/20 flex flex-col gap-1 list-disc pl-4 text-xs text-warning select-text max-h-24 overflow-y-auto">
            {restoreReport.warnings.map((w) => (
              <li key={w}>{w}</li>
            ))}
          </ul>
        )}
      </div>,
    );
  }

  if (ops.restoreError) {
    items.push(
      <ErrorCard
        key="restore-error"
        title="Restore failed"
        message={ops.restoreError}
        onDismiss={ops.dismissRestore}
      />,
    );
  }

  if (items.length === 0) return null;
  return <div className="px-5 pt-4 flex flex-col gap-2 shrink-0">{items}</div>;
}
