import { useId, useState } from "react";

interface Props {
  label: string;
  value: string;
  onChange: (value: string) => void;
  /** Called on Enter. */
  onSubmit?: () => void;
  autoFocus?: boolean;
  /** "new-password" for a passphrase being chosen, "current-password" otherwise. */
  autoComplete?: "new-password" | "current-password";
  hint?: string;
  disabled?: boolean;
}

/** Password field with a show/hide toggle. */
export default function PassphraseInput({
  label,
  value,
  onChange,
  onSubmit,
  autoFocus,
  autoComplete = "current-password",
  hint,
  disabled,
}: Props) {
  const id = useId();
  const hintId = useId();
  const [visible, setVisible] = useState(false);
  return (
    <div className="flex flex-col gap-1.5 w-full">
      <label htmlFor={id} className="text-xs text-muted">
        {label}
      </label>
      <div className="relative">
        <input
          id={id}
          type={visible ? "text" : "password"}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && onSubmit) {
              e.preventDefault();
              onSubmit();
            }
          }}
          autoFocus={autoFocus}
          autoComplete={autoComplete}
          spellCheck={false}
          disabled={disabled}
          aria-describedby={hint ? hintId : undefined}
          className="input w-full pr-14 select-text"
        />
        <button
          type="button"
          onClick={() => setVisible((v) => !v)}
          aria-label={visible ? `Hide ${label.toLowerCase()}` : `Show ${label.toLowerCase()}`}
          aria-pressed={visible}
          className="absolute right-1.5 top-1/2 -translate-y-1/2 px-2 py-1 rounded-lg text-xs text-muted
                     hover:text-white hover:bg-surface-raised transition-colors"
        >
          {visible ? "Hide" : "Show"}
        </button>
      </div>
      {hint && (
        <p id={hintId} className="text-xs text-muted">
          {hint}
        </p>
      )}
    </div>
  );
}
