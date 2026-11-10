# Changelog

Everything that changed in Zen Sync, newest first.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
the project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.0] — 2026-11-10

### Added

- **Backup passphrase and recovery code.** The key that encrypts your
  snapshots is no longer stored readable in your backup repository. On the
  first device you choose a passphrase and get a recovery code; the key is
  stored only locked with those two, so GitHub, a leaked GitHub token, or
  anyone else who can read the repository can't decrypt your backups. Other
  devices ask once for the passphrase or the recovery code.
- **Settings → Encryption** to change the passphrase or make a new recovery
  code. Neither re-encrypts your backups.
- **Start over**, for when both the passphrase and the recovery code are
  lost: deletes every backup so encryption can be set up again.

### Changed

- Snapshots carry a small header naming the key they were made with, so a
  snapshot from an older version or another key is reported as such instead of
  as "wrong key or corrupted bundle".
- Disconnecting GitHub also removes the key from this device.

### Removed

- The unprotected `encryption-key.b64` in the backup repository, and the
  PBKDF2 step it went through, which added nothing for a random key.

### Upgrading

- **Your existing backups are deleted** when you set the passphrase: they were
  made with the old key, which anyone with access to the repository could
  read. Update every device to this version, set the passphrase on one of
  them, enter it on the others, and back up again on each.
- A device still on 0.5 can't restore snapshots made by this version.

## [0.5.2] — 2026-11-10

Fixes from a review of the whole app.

### Fixed

- **Snapshots to keep.** After lowering and then raising the limit, every
  backup overwrote the newest snapshot instead of keeping the history. New
  backups now fill free slots first.
- **A restore that fails part-way puts everything back.** Before, it could
  leave new mods with old settings, or extension data nothing pointed to. Your
  files are restored from the safety copy and the error says so.
- **An unreadable `prefs.js` stops the backup or restore.** A restore used to
  carry on as if the file were empty and rewrite it with only the restored
  prefs, dropping all the others.
- **Starting without a network.** With *Launch at login*, a slow network made
  the app start hidden and signed out. It now shows the window, says it is
  reconnecting, and retries for a few minutes.
- **Quitting or disconnecting mid-operation.** Closing the window during a
  backup or restore now asks first, *Disconnect GitHub* waits until it's done,
  and two operations can no longer run at once.
- **Interrupted uploads** no longer delete the snapshot the slot held before,
  and a backup saved at the same moment as another device's is retried instead
  of failing.
- **Restore picks what you chose.** If another device backed up since the list
  was loaded and the slot now holds a newer snapshot, the restore stops and
  asks you to refresh. Lists also refresh when the window regains focus.
- **Device names with accents or non-Latin letters** (e.g. "Büro-PC") now get
  an id GitHub keeps as-is, so their snapshots can be found again.
- **Extension shortcuts** that can't be restored now come with a warning
  saying why, instead of being skipped silently.
- **Several Zen installs** (e.g. release and Twilight): the profile used most
  recently is picked, not whichever `profiles.ini` lists first.
- **Damaged settings file.** Settings are saved atomically, and a damaged file
  is kept as `state.json.damaged` instead of being silently replaced.
- Renaming the device always keeps its snapshots with it, including during or
  before its first backup.
- Sine's own settings are backed up even when no mods are installed.
- The dashboard's extension count matches what a backup contains.
- Zen running as administrator is detected, and Zen is checked again just
  before a restore writes anything.
- The safety copy fails instead of silently skipping folders it can't read.
- GitHub sign-in checks a `state` value, so a local program can't slip in its
  own account.
- Smaller fixes: a replaced confirm dialog no longer leaves backup and restore
  stuck; Escape closes dialogs wherever focus is; "Yesterday" goes by calendar
  day and relative times stay current; a failed settings save no longer leaves
  the switch and the saved value disagreeing; a selection change that fails to
  save after leaving its screen is reported.

### Notes

- A device whose name has accented or non-Latin letters starts a new set of
  snapshots under its new id. Its earlier snapshots were most likely
  unreachable already.
- The encryption key is still stored in the backup repository next to the
  snapshots. Changing that needs a new way to move the key between devices and
  is planned separately.

## [0.5.1] — 2026-11-10

### Fixed

- **Backups no longer delete themselves once a device's slots are full.**
  When a backup reused a slot (from the fourth backup on, with the default of
  three kept), the clean-up step after the upload deleted the snapshot that had
  just been uploaded. History still listed it, and restoring it failed with
  "Snapshot asset … not found". This has happened since 0.2.0.
- Snapshots that drop off the list after lowering *Snapshots to keep* are now
  deleted from GitHub. Before, only the backup list forgot them.

### Notes

- Snapshots lost this way can't be recovered. They still show in History until
  their slot is reused by a new backup; restoring one fails. Back up again on
  each device to replace them.

## [0.5.0] — 2026-11-09

A pass over the whole interface, plus three fixes it turned up.

### Added

- **Status strip.** Progress and the result of a backup or restore now show at
  the top of every screen. Failures offer **Show logs**, and a finished restore
  tells you Zen can be opened again.
- **"This device" in History.** Snapshots made on the device you are using are
  marked, and only the newest snapshot overall is called *Latest*. History also
  has a **Refresh** link.
- **App version** at the bottom of Settings.
- **Cancel** while waiting for the GitHub sign-in on first run.

### Changed

- **Choose extensions** and **Choose prefs** save each change as you make it,
  like the switches in Settings. The Save button is gone.
- The links to those two screens sit directly under the switches they belong
  to, show how many items are selected, and only appear while the switch is on.
- The `about:config` list is one compact list instead of a card per pref.
  *Select all / none* sits beside the count and acts on the filtered rows when a
  filter is active.
- A backup or restore keeps running, and keeps its result, when you switch
  screens. A second one can't be started until it finishes.
- The restore confirmation focuses **Cancel**, so pressing Enter no longer
  starts a restore.
- Last backup reads as "Yesterday" or "3 hours ago"; hover for the exact time.
  Dates in another year show the year.
- The Inter font ships with the app instead of being fetched from Google Fonts,
  so drawing the window makes no network request.
- Keyboard focus is visible everywhere, switches can be toggled by clicking
  their label, and secondary text has more contrast.
- Error messages can be selected and copied.
- An empty device name is no longer saved.

### Fixed

- **Renaming a device no longer orphans its snapshots.** Snapshots were filed
  under an id derived from the device name, so after a rename the earlier ones
  were left under the old name and never pruned. A device that has backups now
  keeps its id when renamed.
- **A failed update can be retried.** The pending update was discarded on the
  first attempt, so **Install** failed with "No pending update" the second time.
  Install failures are also shown in the banner instead of being logged only.
- **An abandoned GitHub sign-in no longer waits forever.** It gives up after
  five minutes and stops listening for the browser's reply.
- The dashboard's "Snapshots stored" showed the *Snapshots to keep* setting, not
  a count. It is now labelled "Snapshots kept per device".
- History showed a bare "Error" with no way to retry when the list failed to
  load.

### Removed

- Mentions of a system tray icon. The app does not have one; *Launch at login*
  opens the window.

### Notes

- Snapshots already orphaned by a rename under 0.4.0 or earlier stay where they
  are. They still appear in History under the old name and can be restored.
- Two devices with the same name still share one set of snapshots. Give each
  device its own name.

## [0.4.0] — 2026-09-19

### Added

- **Zen keyboard shortcuts.** The keys you rebind in Zen's Keyboard Shortcuts
  panel — switching workspaces, compact mode, split view and the rest — now
  travel with your snapshots. Mozilla sync doesn't carry them. On by default.
- **`about:config` prefs.** Settings you changed in `about:config` can now be
  backed up and restored. Off until you turn it on in Settings, and a new
  **Choose prefs** screen lists every pref it would carry so you can drop
  individual ones before the first backup.

### Notes

- Only prefs that differ from their default are on offer, because those are the
  only ones Zen writes to `prefs.js`. Prefs that describe the machine rather
  than your configuration — hardware and codec state, printers, sessions,
  telemetry, download folders, your Mozilla account, anything holding a local
  path — are never offered, and neither are prefs another sync option already
  owns.
- A restore only *sets* the prefs a snapshot carries; it never clears one. A
  pref sitting at its default is simply absent from `prefs.js`, so a pref you
  reset on another device and one you never set there look identical. Prefs you
  changed only on this device are left alone.
- Zen records its shortcut schema version in a pref rather than in the shortcuts
  file, so a restore writes whichever version is lower — the snapshot's or this
  device's. Claiming a version newer than the local Zen would make it discard
  the restored shortcuts and rebuild them from defaults.
- Snapshot format stays at 3. Both additions are optional sections that 0.3.x
  ignores, so it can still restore snapshots made by 0.4.0 — just without the
  shortcuts and prefs.
- Keyboard shortcuts were part of the full-profile backups in 0.1.x and were
  dropped by the 0.2.0 scope reduction. This brings them back deliberately, as
  their own option.

## [0.3.1] — 2026-09-15

### Changed

- The main window lost its native frame. Every screen's header now drags the
  window and ends in a close button, and Windows 11 is asked to round the
  corners. The update banner moved below the header so the close button stays
  in the corner.

### Fixed

- The release script bumps `Cargo.lock` along with the other version files, so a
  build no longer leaves it modified.

## [0.3.0] — 2026-09-15

### Added

- **Per-device sync options.** Settings gained switches for Sine mods, mod
  settings, extension data, permissions and shortcuts. Each device decides what
  it contributes to backups and what it takes from restores.
- **Restore latest backup** on the dashboard, next to Back up now — the newest
  snapshot from any of your devices, without opening the History screen.
- **In-window confirmation dialogs.** The browser's own `confirm()` could open
  outside the small app window.

### Changed

- Snapshot format 2 → 3, because a backup can now leave out mod files or mod
  settings and 0.2.x would have handled that by wiping the mods folder. Format 2
  snapshots still restore; 0.2.x refuses format 3 rather than damaging anything.
- A restore no longer resets mod settings the source device never declared, so
  settings belonging to mods only this device has survive.
- The "is Zen running" check re-runs every 2 seconds instead of only when a
  screen opens, and reads just process names and owners, off the main thread.

## [0.2.1] — 2026-09-15

### Added

- **Portable build.** A standalone `.exe` that needs no installer and shares its
  settings and GitHub connection with an installed copy. Its update banner opens
  the release page instead of running the installer, which would leave you with
  a second, installed copy.

## [0.2.0] — 2026-09-15

Zen's Mozilla account sync now covers spaces, containers, bookmarks and
`storage.sync`, so Zen Sync stopped duplicating it and shrank to what it still
misses. This is a breaking change to what a snapshot contains.

### Changed

- **Snapshots carry only Sine mods and extension data.** `chrome/sine-mods/`
  minus Sine's generated stylesheets (they embed absolute profile paths), plus
  the `prefs.js` values of every setting the mods and Sine declare.
- Snapshots are zip archives (format 2), stored as `mods-*.enc` with
  `metadata-v2.json`.
- Pre-restore safety copies moved out of the profile into the app config dir.

### Added

- Extension `storage.local` for the extensions you select, retargeted to the
  local extension UUID on restore, plus the permissions you granted and each
  extension's custom shortcuts.
- Password-manager extensions are excluded by default, because their local
  storage holds your account session. You can opt them back in.

### Removed

- Full-profile backups. Everything Zen's own sync covers is left to it.

### Upgrading

The first backup after upgrading deletes your 0.1.x full-profile snapshots from
GitHub, after asking. They cannot be restored by 0.2.0 or later.

## [0.1.3] — 2026-06-08

No user-facing changes — adds the release script used to cut versions.

## [0.1.2] — 2026-06-08

### Added

- Acknowledgement of [Zync](https://github.com/jessewallace/zync) by Jesse
  Wallace, the tool Zen Sync was built as a simpler, manual alternative to.

### Changed

- New application icon.

## [0.1.1] — 2026-06-08

### Added

- Mod preferences are included in backups: `chrome/userChrome.css` and every
  per-mod file under `chrome/zen-themes/`, including each mod's
  `preferences.json`.

## [0.1.0] — 2026-06-05

First release. Encrypted, on-demand backups of your Zen Browser profile to a
private GitHub repository.

### Added

- Manual backup and restore — no background daemon, no automatic triggers, and
  no network activity unless you click a button.
- AES-256-GCM encryption with PBKDF2-HMAC-SHA256 key derivation (100k rounds).
  GitHub never sees plaintext data.
- The encryption key lives in Windows Credential Manager, never on disk.
- Backups cover workspaces and pinned tabs (`places.sqlite`), preferences,
  extensions, mod configuration and styles, keyboard shortcuts, sessions, live
  folders and container settings. Passwords and session cache are never
  included.
- Device-name isolation: `services.sync.*` and `identity.fxaccounts.*` are
  stripped before upload and re-injected from the local device, so each machine
  keeps its own Firefox Sync identity.
- Per-extension selection for backups.
- Snapshot history, 3 per device by default and configurable from 1 to 10, with
  restore from any device onto any other.
- Hard block on backing up or restoring while Zen Browser is running, which
  prevents the database corruption and browser-state weirdness that comes of
  replacing profile files the browser holds open.
- In-app update banner, system tray icon, and autostart (off by default).

[0.6.0]: https://github.com/PixelPusher247/zen-sync/compare/v0.5.2...v0.6.0
[0.5.2]: https://github.com/PixelPusher247/zen-sync/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/PixelPusher247/zen-sync/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/PixelPusher247/zen-sync/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/PixelPusher247/zen-sync/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/PixelPusher247/zen-sync/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/PixelPusher247/zen-sync/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/PixelPusher247/zen-sync/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/PixelPusher247/zen-sync/compare/v0.1.3...v0.2.0
[0.1.3]: https://github.com/PixelPusher247/zen-sync/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/PixelPusher247/zen-sync/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/PixelPusher247/zen-sync/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/PixelPusher247/zen-sync/releases/tag/v0.1.0
