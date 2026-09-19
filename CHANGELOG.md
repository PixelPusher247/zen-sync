# Changelog

Everything that changed in Zen Sync, newest first.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
the project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[0.4.0]: https://github.com/PixelPusher247/zen-sync/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/PixelPusher247/zen-sync/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/PixelPusher247/zen-sync/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/PixelPusher247/zen-sync/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/PixelPusher247/zen-sync/compare/v0.1.3...v0.2.0
[0.1.3]: https://github.com/PixelPusher247/zen-sync/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/PixelPusher247/zen-sync/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/PixelPusher247/zen-sync/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/PixelPusher247/zen-sync/releases/tag/v0.1.0
