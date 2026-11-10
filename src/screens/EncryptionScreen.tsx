import { useState } from "react";
import { bridge } from "../bridge";
import PickerHeader from "../components/PickerHeader";
import PassphraseInput from "../components/PassphraseInput";
import RecoveryCodeCard from "../components/RecoveryCodeCard";
import { MIN_PASSPHRASE_CHARS, passphraseProblem } from "./KeyScreen";

interface Props {
  onBack: () => void;
}

function Spinner() {
  return <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />;
}

/** Settings → change the passphrase or make a new recovery code. */
export default function EncryptionScreen({ onBack }: Props) {
  return (
    <div className="flex flex-col h-full animate-fade-in">
      <PickerHeader
        title="Encryption"
        subtitle="Passphrase and recovery code"
        saveState="idle"
        onBack={onBack}
      />
      <div className="flex-1 min-h-0 overflow-y-auto p-4 flex flex-col gap-4">
        <p className="text-xs text-muted">
          Your backups are encrypted with a key stored in the backup repository
          only in locked form. Your passphrase or your recovery code unlocks it
          on a new device. Changing either doesn't touch existing backups.
        </p>
        <ChangePassphrase />
        <NewRecoveryCode />
      </div>
    </div>
  );
}

function ChangePassphrase() {
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [busy, setBusy] = useState(false);
  const [attempted, setAttempted] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState(false);

  const problem = passphraseProblem(next, confirmation);

  async function submit() {
    setAttempted(true);
    setDone(false);
    if (!current || problem || busy) return;
    setBusy(true);
    setError(null);
    try {
      await bridge.changePassphrase(current, next);
      setCurrent("");
      setNext("");
      setConfirmation("");
      setAttempted(false);
      setDone(true);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <form
      className="card p-4 flex flex-col gap-3"
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <p className="section-label mb-0">Change passphrase</p>
      <PassphraseInput label="Current passphrase" value={current} onChange={setCurrent} disabled={busy} />
      <PassphraseInput
        label="New passphrase"
        value={next}
        onChange={setNext}
        autoComplete="new-password"
        hint={`At least ${MIN_PASSPHRASE_CHARS} characters.`}
        disabled={busy}
      />
      <PassphraseInput
        label="Repeat new passphrase"
        value={confirmation}
        onChange={setConfirmation}
        onSubmit={submit}
        autoComplete="new-password"
        disabled={busy}
      />
      {attempted && !current && <p className="text-xs text-warning">Enter your current passphrase.</p>}
      {attempted && current && problem && <p className="text-xs text-warning">{problem}</p>}
      {error && (
        <p role="alert" className="text-xs text-danger select-text break-words">
          {error}
        </p>
      )}
      {done && (
        <p role="status" className="text-xs text-success">
          ✓ Passphrase changed. Use the new one on your next device.
        </p>
      )}
      <button type="submit" disabled={busy} className="btn-secondary text-xs w-full">
        {busy ? (
          <>
            <Spinner />
            Changing…
          </>
        ) : (
          "Change passphrase"
        )}
      </button>
    </form>
  );
}

function NewRecoveryCode() {
  const [passphrase, setPassphrase] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [code, setCode] = useState<string | null>(null);

  async function submit() {
    if (!passphrase || busy) return;
    setBusy(true);
    setError(null);
    try {
      setCode(await bridge.newRecoveryCode(passphrase));
      setPassphrase("");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  if (code) {
    return (
      <div className="card p-4 flex flex-col gap-3">
        <p className="section-label mb-0">New recovery code</p>
        <p className="text-xs text-subtle">Your old recovery code no longer works.</p>
        <RecoveryCodeCard code={code} doneLabel="Done" onDone={() => setCode(null)} />
      </div>
    );
  }

  return (
    <form
      className="card p-4 flex flex-col gap-3"
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <div>
        <p className="section-label mb-0.5">Recovery code</p>
        <p className="text-xs text-muted">
          Lost your recovery code, or think someone saw it? Make a new one; the
          old one stops working.
        </p>
      </div>
      <PassphraseInput
        label="Passphrase"
        value={passphrase}
        onChange={setPassphrase}
        onSubmit={submit}
        disabled={busy}
      />
      {error && (
        <p role="alert" className="text-xs text-danger select-text break-words">
          {error}
        </p>
      )}
      <button type="submit" disabled={busy || !passphrase} className="btn-secondary text-xs w-full">
        {busy ? (
          <>
            <Spinner />
            Making a new code…
          </>
        ) : (
          "Make a new recovery code"
        )}
      </button>
    </form>
  );
}
