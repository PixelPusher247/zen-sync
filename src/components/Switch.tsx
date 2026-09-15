interface Props {
  checked: boolean;
  label: string;
  description?: string;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
}

/** Labelled on/off row used on the settings screen. */
export default function Switch({ checked, label, description, disabled, onChange }: Props) {
  return (
    <div className="flex items-center justify-between gap-3">
      <div className="min-w-0">
        <p className="text-sm text-white">{label}</p>
        {description && <p className="text-xs text-muted mt-0.5">{description}</p>}
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={checked ? "true" : "false"}
        aria-label={label}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={`
          relative w-11 h-6 shrink-0 rounded-full transition-colors duration-200 appearance-none outline-none
          disabled:opacity-40 disabled:cursor-not-allowed
          ${checked ? "bg-accent" : "bg-surface-border"}
        `}
      >
        <span className={checked ? "toggle-thumb-on" : "toggle-thumb-off"} />
      </button>
    </div>
  );
}
