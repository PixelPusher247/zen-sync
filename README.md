<div align="center">

<img src="src-tauri/icons/128x128.png" width="96" alt="Zen Sync icon" />

# Zen Sync

**Encrypted backups of your Zen Browser mods and extension data, on demand.**

[![Version](https://img.shields.io/github/v/release/PixelPusher247/zen-sync?style=flat-square&color=7c6af7&label=version)](https://github.com/PixelPusher247/zen-sync/releases/latest)
[![Platform](https://img.shields.io/badge/platform-Windows-0078d4?style=flat-square)](#installation)
[![License](https://img.shields.io/badge/license-MIT-7c6af7?style=flat-square)](LICENSE)

</div>

---

[Zen Browser](https://www.zen-browser.app/) syncs spaces, containers, bookmarks and extension `storage.sync` data through your Mozilla account. A few things are still left out: [Sine](https://github.com/CosmoCreeper/Sine) mods with their settings, the settings extensions keep in `storage.local`, the shortcuts you rebind in Zen's Keyboard Shortcuts panel, and anything you change in `about:config`. Zen Sync backs up exactly those to a private GitHub repository, encrypted, whenever you choose.

Unlike [Zync](https://github.com/PixelPusher247/zync), there are no background daemons, no automatic triggers, and no network activity unless you click a button.

---

## How it works

```
┌───────────────┐      encrypt      ┌──────────────────────┐
│  Your profile │ ────────────────▶ │  GitHub Release asset │
│  (Zen Browser)│  AES-256-GCM      │  zen-sync-backup repo │
└───────────────┘                   └──────────────────────┘
        ▲                                      │
        │          decrypt + merge             │
        └──────────────────────────────────────┘
                     on restore
```

1. **Backup** — Click "Back up now". Zen Sync zips your Sine mods, the values of the settings those mods declare, and the data of your selected extensions, encrypts the archive with AES-256-GCM, and uploads it as a release asset to a private `zen-sync-backup` repo in your GitHub account.
2. **Restore** — Click "Restore latest backup" to restore the newest snapshot from any device, or open the History screen to pick an older one. Your mods folder is replaced with the snapshot's, mod settings are written into `prefs.js` one by one, and extension storage is copied in. Extension storage is tied to a per-profile UUID, so Zen Sync rewrites it to match the UUID the extension has on this device. Zen's shortcuts file is replaced and the `about:config` prefs you picked are written in. Nothing else in `prefs.js` is touched, so device name and Mozilla account stay as they are.
3. **Zen must be closed** — All operations are hard-blocked if Zen Browser is running, preventing database corruption and conflicting writes to files the browser holds open. The app notices within a couple of seconds when you close or open Zen.
4. **Your passphrase unlocks the key** — On the first device you choose a backup passphrase and get a recovery code. The key that encrypts your snapshots is stored in the repository only locked with those two, so GitHub, or anyone else who can read the repository, can't decrypt your backups. Each further device asks once for the passphrase (or the recovery code) and then keeps the key in Windows Credential Manager.

---

## Features

- **Manual only** — no background sync, no auto-triggers, no network use at rest
- **AES-256-GCM encryption** under a random key that only your passphrase or recovery code can unlock; GitHub never sees plaintext data or a usable key
- **Passphrase and recovery code** — enter one of them once per device; change the passphrase or make a new recovery code in Settings without re-encrypting anything
- **Works alongside Mozilla sync** — only data native sync doesn't cover is backed up; device identity and sync-account prefs are never read or written
- **Choose what to sync** — turn Sine mods, mod settings, extension data, permissions, shortcuts, Zen's keyboard shortcuts and `about:config` prefs on or off per device; the choice applies to both backups and restores on that device
- **Per-pref selection** — `about:config` sync is off until you turn it on, and the prefs screen lists every pref it would carry so you can drop individual ones; prefs that describe the machine are never offered
- **Per-extension selection** — choose which extensions' data to include; password managers are excluded by default because their local storage holds your account session
- **Snapshot history** — keeps the last N snapshots per device (default 3, configurable 1–10); restore any of them from the History screen
- **Cross-device restore** — snapshots from all your devices appear in the History screen; you can restore any device's backup onto any other device
- **Auto-updater** — in-app update banner when a new version is released
- **Launch at login** — off by default, toggleable in Settings

---

## What gets synced

| Data | Source |
|------|--------|
| Sine mods | `chrome/sine-mods/` (except `chrome.css` / `content.css`, which Sine regenerates on startup) |
| Mod settings | Values in `prefs.js` for every `property` declared in a mod's `preferences.json`, plus Sine's own settings |
| Extension storage | `storage/default/moz-extension+++<uuid>^userContextId=4294967295/` (`storage.local`) |
| Extension permissions | The extension's entry in `extension-preferences.json` |
| Extension shortcuts | The extension's entries under `commands` in `extension-settings.json` |
| Zen shortcuts | `zen-keyboard-shortcuts.json`, with the `zen.keyboard.shortcuts.version` pref that says which schema it is in |
| `about:config` prefs | The prefs in `prefs.js` you selected on the prefs screen |

**Left to Mozilla sync:** spaces, containers, bookmarks, history, passwords, installed extensions, `storage.sync`. The Sine engine itself (`chrome/JS/`) must be installed on each device.

**Never synced:** prefs that belong to the machine rather than to you — hardware and codec state, printers, sessions and profile databases, telemetry and update bookkeeping, download folders, your Mozilla account and device name, and any pref whose value is a local path. Extension state is left to the extension options above, and mod settings to Mod settings, so no pref has two owners.

A restore only writes the prefs a snapshot carries. A pref you changed on this device but not on the one that made the snapshot is left alone, because a pref sitting at its default is absent from `prefs.js` either way and the two cases can't be told apart.

---

## Installation

Download the latest Windows installer from the [Releases](https://github.com/PixelPusher247/zen-sync/releases/latest) page:

| Format | Notes |
|--------|-------|
| `.exe` (NSIS) | Recommended — standard Windows installer |
| `.msi` | For enterprise / group policy deployments |
| `_x64-portable.exe` | No installer — just run it. Needs the WebView2 runtime (built into Windows 11). Updates are downloaded manually from the in-app banner. Settings and the GitHub connection are shared with an installed copy. |

> Windows may show a SmartScreen prompt on first run for unsigned builds. Click **More info → Run anyway** to proceed.

---

## Security

- Snapshots are encrypted with **AES-256-GCM** under a random 256-bit key before leaving your machine
- That key is stored in the repository (`encryption-v2.json`) only **wrapped**: once under a key derived from your passphrase with **Argon2id** (64 MiB, 3 passes), and once under one derived from your recovery code. Anyone who can read the repository still needs one of the two
- On each device the unwrapped key is kept in **Windows Credential Manager**; disconnecting GitHub removes it
- The recovery code is 25 random characters (125 bits) and is shown only once. If you lose both it and the passphrase, nobody can decrypt your backups; the only way on is **Start over**, which deletes them
- Passphrases are at least 10 characters. A long one matters: a copy of the key file lets someone try guesses offline, and Argon2id only makes each guess slow
- The backup repository is **private** and owned by your GitHub account
- Before every restore, the files it will replace are copied to `%APPDATA%\app.zen.zensync\restore-backups\{timestamp}\` (the last 3 are kept), giving you a manual rollback path independent of the GitHub history
- Upgrading to 0.6: Zen Sync 0.5 and earlier uploaded the key unprotected, next to the snapshots. Setting a passphrase in 0.6 deletes that key and every backup made with it, so update all your devices and back up again
- Upgrading from 0.1.x: the first backup deletes the old full-profile snapshots from GitHub after asking for confirmation

---

## Building from source

See **[SETUP.md](SETUP.md)** for the full step-by-step guide covering:

- Registering a GitHub OAuth App
- Setting environment variables for local builds
- Generating the updater signing key
- Adding GitHub repository secrets for CI releases
- Running locally and publishing a release

Quick start (after completing SETUP.md steps 1–3):

```powershell
npm install
npm run dev      # development build with hot-reload
npm run build    # production installer → src-tauri/target/release/bundle/
```

**Requirements:** Rust stable · Node.js 20+ · Windows 10/11

---

## Comparison with Zync

| | Zync | Zen Sync |
|---|---|---|
| Sync trigger | Automatic (on Zen close) + manual | Manual only |
| Storage | GitHub Releases | GitHub Releases |
| Platform | macOS, Windows, Linux | Windows |
| Background daemons | Yes (Zen watcher, ntfy poller) | None |
| Real-time notifications | ntfy.sh | — |
| Browser-open guard | Warn | Hard block |
| Device name on restore | Overwritten | Preserved |
| Extension selection | All or nothing | Per-extension toggle |
| Autostart | Enabled by default | Disabled by default |

---

## Acknowledgements

Zen Sync was inspired by [Zync](https://github.com/jessewallace/zync) by Jesse Wallace — a full-featured Zen Browser sync tool with automatic triggers and cross-platform support. Zen Sync was built in search of a simpler, manual push/pull alternative without background daemons or auto-sync.

---

## License

MIT — see [LICENSE](LICENSE).
