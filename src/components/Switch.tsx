import { useId } from "react";

interface Props {
  checked: boolean;
  label: string;
  description?: string;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
}

/** Labelled on/off row used on the settings screen. Clicking the text toggles it too. */
export default function Switch({ checked, label, description, disabled, onChange }: Props) {
  const id = useId();
  const descriptionId = useId();
  return (
    <div className="flex items-center justify-between gap-3">
      <label
        htmlFor={id}
        className={`min-w-0 flex-1 ${disabled ? "cursor-not-allowed" : "cursor-pointer"}`}
      >
        <span className="block text-sm text-white">{label}</span>
        {description && (
          <span id={descriptionId} className="block text-xs text-muted mt-0.5">
            {description}
          </span>
        )}
      </label>
      <button
        id={id}
        type="button"
        role="switch"
        aria-checked={checked ? "true" : "false"}
        aria-describedby={description ? descriptionId : undefined}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={`
          relative w-11 h-6 shrink-0 rounded-full transition-colors duration-200 appearance-none
          disabled:opacity-40 disabled:cursor-not-allowed
          ${checked ? "bg-accent" : "bg-surface-border"}
        `}
      >
        <span className={checked ? "toggle-thumb-on" : "toggle-thumb-off"} />
      </button>
    </div>
  );
}
