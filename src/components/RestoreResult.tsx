import type { RestoreReport } from "../types";

interface Props {
  report: RestoreReport;
  onDismiss: () => void;
}

function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}

export default function RestoreResult({ report, onDismiss }: Props) {
  const parts = [
    report.modCount > 0 && plural(report.modCount, "mod"),
    report.extensionCount > 0 && plural(report.extensionCount, "extension"),
    report.shortcutsRestored && "Zen shortcuts",
    report.prefCount > 0 && plural(report.prefCount, "pref"),
  ].filter(Boolean);

  return (
    <div className="flex flex-col gap-2 animate-fade-in">
      <div className="flex items-center gap-2 p-3 bg-success/10 border border-success/20 rounded-xl text-sm text-success">
        <span>✓</span>
        <span>{parts.length > 0 ? `Restored ${parts.join(" and ")}` : "Restore complete"}</span>
      </div>
      {report.warnings.length > 0 && (
        <div className="p-3 bg-warning/10 border border-warning/20 rounded-xl text-xs text-warning">
          <ul className="flex flex-col gap-1 list-disc pl-4">
            {report.warnings.map((w) => (
              <li key={w}>{w}</li>
            ))}
          </ul>
          <button
            type="button"
            onClick={onDismiss}
            className="mt-2 underline opacity-70 hover:opacity-100"
          >
            Dismiss
          </button>
        </div>
      )}
    </div>
  );
}
