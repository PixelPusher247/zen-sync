import { useState } from "react";

interface Props {
  code: string;
  /** Label of the button that continues once the code is saved. */
  doneLabel: string;
  onDone: () => void;
}

/**
 * Shows a recovery code once, with a copy button, and only lets the user go
 * on after they confirm they have saved it: the app never shows it again.
 */
export default function RecoveryCodeCard({ code, doneLabel, onDone }: Props) {
  const [copied, setCopied] = useState(false);
  const [copyFailed, setCopyFailed] = useState(false);
  const [saved, setSaved] = useState(false);

  async function copy() {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      setCopyFailed(false);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      setCopyFailed(true);
    }
  }

  return (
    <div className="w-full flex flex-col gap-3">
      <div className="card p-4 flex flex-col items-center gap-3">
        <p
          className="font-mono text-sm text-white tracking-wide text-center whitespace-nowrap select-all"
          aria-label={`Recovery code ${code.split("").join(" ")}`}
        >
          {code}
        </p>
        <button type="button" onClick={copy} className="btn-secondary text-xs px-3 py-1.5">
          {copied ? "✓ Copied" : "Copy code"}
        </button>
        {copyFailed && (
          <p className="text-xs text-warning text-center">
            Couldn't copy. Select the code and copy it, or write it down.
          </p>
        )}
      </div>

      <p className="text-xs text-muted">
        Keep it somewhere safe outside this PC, such as a password manager or
        on paper. This is the only time it is shown.
      </p>

      <label className="flex items-start gap-2.5 text-sm text-white cursor-pointer">
        <input
          type="checkbox"
          checked={saved}
          onChange={(e) => setSaved(e.target.checked)}
          className="mt-0.5 w-4 h-4 accent-accent shrink-0"
        />
        I've saved my recovery code
      </label>

      <button type="button" onClick={onDone} disabled={!saved} className="btn-primary w-full py-2.5">
        {doneLabel}
      </button>
    </div>
  );
}
