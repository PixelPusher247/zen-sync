import { useState } from "react";
import { bridge } from "../bridge";
import type { AppStatus, SetupEncryptionResult } from "../types";
import { FloatingTitleBar } from "../components/WindowControls";
import PassphraseInput from "../components/PassphraseInput";
import RecoveryCodeCard from "../components/RecoveryCodeCard";
import { useConfirm } from "../components/ConfirmDialog";

/** Keep in step with `MIN_PASSPHRASE_CHARS` in keyfile.rs. */
export const MIN_PASSPHRASE_CHARS = 10;

interface Props {
  status: AppStatus;
  onStatusChange: (status: AppStatus) => void;
}

/** Problem with a new passphrase and its confirmation, or null if fine. */
export function passphraseProblem(passphrase: string, confirmation: string): string | null {
  if ([...passphrase].length < MIN_PASSPHRASE_CHARS) {
    return `Use at least ${MIN_PASSPHRASE_CHARS} characters.`;
  }
  if (passphrase !== confirmation) return "The two passphrases don't match.";
  return null;
}

function Spinner() {
  return <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />;
}

/**
 * Shown after connecting to GitHub until this device has the snapshot key:
 * choose a passphrase on the first device, or unlock on the others.
 */
export default function KeyScreen({ status, onStatusChange }: Props) {
  return (
    <div className="relative h-full overflow-y-auto bg-surface animate-fade-in">
      <FloatingTitleBar />
      <div className="min-h-full flex flex-col items-center justify-center px-8 pt-12 pb-8">
        <div className="w-full max-w-xs flex flex-col items-center gap-5">
          <img src="/app-icon.svg" alt="" className="w-12 h-12 rounded-xl" draggable={false} />
          {status.encryption === "locked" ? (
            <Unlock onStatusChange={onStatusChange} />
          ) : status.encryption === "needs-setup" ? (
            <Setup oldBackups={status.oldBackups === true} onStatusChange={onStatusChange} />
          ) : (
            <p className="text-sm text-subtle" role="status">
              Checking your backup encryption…
            </p>
          )}
        </div>
      </div>
    </div>
  );
}

function Setup({
  oldBackups,
  onStatusChange,
}: {
  oldBackups: boolean;
  onStatusChange: (status: AppStatus) => void;
}) {
  const [passphrase, setPassphrase] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [attempted, setAttempted] = useState(false);
  // Held until the user confirms the code is saved; applying the new status
  // earlier would leave this screen before the code was seen.
  const [result, setResult] = useState<SetupEncryptionResult | null>(null);

  const problem = passphraseProblem(passphrase, confirmation);

  async function submit() {
    setAttempted(true);
    if (problem || busy) return;
    setBusy(true);
    setError(null);
    try {
      setResult(await bridge.setupEncryption(passphrase));
      setPassphrase("");
      setConfirmation("");
    } catch (e) {
      setError(String(e));
      // Another device may have set up encryption first.
      bridge.getStatus().then((s) => s.encryption !== "needs-setup" && onStatusChange(s)).catch(() => {});
    } finally {
      setBusy(false);
    }
  }

  if (result) {
    return (
      <>
        <div className="text-center">
          <h1 className="text-lg font-bold text-white">Save your recovery code</h1>
          <p className="text-subtle text-xs mt-1.5">
            If you forget your passphrase, this code is the only other way to
            unlock your backups.
          </p>
        </div>
        {result.warning && (
          <div role="alert" className="w-full p-3 bg-warning/10 border border-warning/20 rounded-xl text-xs text-warning select-text break-words">
            {result.warning}
          </div>
        )}
        <RecoveryCodeCard
          code={result.recoveryCode}
          doneLabel="Continue"
          onDone={() => onStatusChange(result.status)}
        />
      </>
    );
  }

  return (
    <>
      <div className="text-center">
        <h1 className="text-lg font-bold text-white">Choose a backup passphrase</h1>
        <p className="text-subtle text-xs mt-1.5">
          Your backups are encrypted with a key only this passphrase or a
          recovery code can unlock: not GitHub, and not anyone else with
          access to the repository. You enter it once on each device.
        </p>
      </div>

      {oldBackups && (
        <div className="w-full p-3 bg-warning/10 border border-warning/20 rounded-xl text-xs text-warning">
          Backups made with Zen Sync 0.5 or earlier will be deleted. Their key
          was stored unprotected in the repository, so they can't be kept
          safely. Back up again afterwards.
        </div>
      )}

      <form
        className="w-full flex flex-col gap-3"
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <PassphraseInput
          label="Passphrase"
          value={passphrase}
          onChange={setPassphrase}
          autoFocus
          autoComplete="new-password"
          hint={`At least ${MIN_PASSPHRASE_CHARS} characters. A few unrelated words work well.`}
          disabled={busy}
        />
        <PassphraseInput
          label="Repeat passphrase"
          value={confirmation}
          onChange={setConfirmation}
          onSubmit={submit}
          autoComplete="new-password"
          disabled={busy}
        />
        {attempted && problem && <p className="text-xs text-warning">{problem}</p>}
        {error && (
          <div role="alert" className="p-3 bg-danger/10 border border-danger/20 rounded-xl text-xs text-danger select-text break-words">
            {error}
          </div>
        )}
        <button type="submit" disabled={busy} className="btn-primary w-full py-2.5">
          {busy ? (
            <>
              <Spinner />
              Setting up encryption…
            </>
          ) : (
            "Set passphrase"
          )}
        </button>
      </form>
      <p className="text-xs text-muted text-center">
        Forgetting both the passphrase and the recovery code means the backups
        can't be recovered by anyone.
      </p>
    </>
  );
}

function Unlock({ onStatusChange }: { onStatusChange: (status: AppStatus) => void }) {
  const confirm = useConfirm();
  const [secret, setSecret] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit() {
    if (!secret.trim() || busy) return;
    setBusy(true);
    setError(null);
    try {
      onStatusChange(await bridge.unlockEncryption(secret));
    } catch (e) {
      setError(String(e));
      // Encryption may have been reset from another device meanwhile.
      bridge.getStatus().then((s) => s.encryption !== "locked" && onStatusChange(s)).catch(() => {});
    } finally {
      setBusy(false);
    }
  }

  async function startOver() {
    const ok = await confirm({
      title: "Delete all backups and start over?",
      message: (
        <>
          <p>
            Without the passphrase or the recovery code, nobody can decrypt
            your backups, including Zen Sync.
          </p>
          <p className="mt-2">
            Starting over deletes every backup from GitHub and lets you choose
            a new passphrase. Your other devices will need it too.
          </p>
        </>
      ),
      confirmLabel: "Delete backups",
      danger: true,
    });
    if (!ok) return;
    setBusy(true);
    setError(null);
    try {
      onStatusChange(await bridge.resetEncryption());
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function disconnect() {
    try {
      await bridge.disconnectGithub();
      onStatusChange(await bridge.getStatus());
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <>
      <div className="text-center">
        <h1 className="text-lg font-bold text-white">Unlock your backups</h1>
        <p className="text-subtle text-xs mt-1.5">
          Enter the backup passphrase you chose when you set up Zen Sync, or
          your recovery code.
        </p>
      </div>
      <form
        className="w-full flex flex-col gap-3"
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <PassphraseInput
          label="Passphrase or recovery code"
          value={secret}
          onChange={setSecret}
          onSubmit={submit}
          autoFocus
          disabled={busy}
        />
        {error && (
          <div role="alert" className="p-3 bg-danger/10 border border-danger/20 rounded-xl text-xs text-danger select-text break-words">
            {error}
          </div>
        )}
        <button type="submit" disabled={busy || !secret.trim()} className="btn-primary w-full py-2.5">
          {busy ? (
            <>
              <Spinner />
              Unlocking…
            </>
          ) : (
            "Unlock"
          )}
        </button>
      </form>
      <div className="flex flex-col items-center gap-1.5 text-xs">
        <button type="button" onClick={startOver} disabled={busy} className="text-muted hover:text-white disabled:opacity-40">
          Forgot both? Start over
        </button>
        <button type="button" onClick={disconnect} disabled={busy} className="text-muted hover:text-white disabled:opacity-40">
          Use a different GitHub account
        </button>
      </div>
    </>
  );
}
