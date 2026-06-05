<div align="center">

<img src="src-tauri/icons/128x128.png" width="96" alt="Zen Sync icon" />

# Zen Sync

**Encrypted Zen Browser profile backups, on demand.**

[![Version](https://img.shields.io/badge/version-0.1.0-7c6af7?style=flat-square)](https://github.com/YOUR_USERNAME/zen-sync/releases/latest)
[![Platform](https://img.shields.io/badge/platform-Windows-0078d4?style=flat-square)](#installation)
[![License](https://img.shields.io/badge/license-MIT-7c6af7?style=flat-square)](LICENSE)

</div>

---

[Zen Browser](https://www.zen-browser.app/) stores workspaces, pinned tabs, themes, and shortcuts in its own data — none of which Firefox Sync covers. Zen Sync backs all of that up to a private GitHub repository, encrypted, whenever you choose.

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

1. **Backup** — Click "Backup Now". Zen Sync collects your profile files, strips per-machine keys from `prefs.js`, filters to your chosen extensions, encrypts everything with AES-256-GCM, and uploads the bundle as a release asset to a private `zen-sync-backup` repo in your GitHub account.
2. **Restore** — Open the History screen, pick any snapshot from any device, and click Restore. The bundle is downloaded, decrypted, and written to your profile. Per-machine preferences (device name, Firefox Sync account) are re-injected from the local device so they are never overwritten.
3. **Zen must be closed** — All operations are hard-blocked if Zen Browser is running, preventing SQLite corruption and the browser-state weirdness that can occur when profile files are replaced while the browser holds them open.

---

## Features

- **Manual only** — no background sync, no auto-triggers, no network use at rest
- **AES-256-GCM encryption** with PBKDF2-HMAC-SHA256 key derivation (100 k rounds); GitHub never sees plaintext data
- **Encryption key in OS keychain** — stored in Windows Credential Manager, never on disk
- **Device name isolation** — `services.sync.*` and `identity.fxaccounts.*` keys are stripped before upload and restored from the local device, so each machine keeps its own Firefox Sync identity
- **Per-extension selection** — choose exactly which extensions to include in backups; non-selected extensions on the target device are left untouched
- **Snapshot history** — keeps the last N snapshots per device (default 3, configurable 1–10); restore any of them from the History screen
- **Cross-device restore** — snapshots from all your devices appear in the History screen; you can restore any device's backup onto any other device
- **Auto-updater** — in-app update banner when a new version is released
- **Tray icon** — runs in the system tray; autostart disabled by default, toggleable from the tray or Settings

---

## What gets synced

| File | Contents |
|------|----------|
| `places.sqlite` | Pinned tabs, workspaces, bookmarks |
| `prefs.js` | Browser preferences — per-machine keys excluded |
| `extensions.json` | Extension list — selectable per extension |
| `zen-themes.json` | Mods/themes configuration |
| `zen-keyboard-shortcuts.json` | Keyboard shortcuts |
| `zen-sessions.jsonlz4` | Workspace names, tab assignments, themes |
| `zen-live-folders.jsonlz4` | Live folders |
| `chrome/zen-themes.css` | Compiled active mod styles |
| `containers.json` | Workspace icons and colors |

**Never synced:** passwords (`key4.db`, `logins.json`), extension storage, session cache.

---

## Installation

Download the latest Windows installer from the [Releases](https://github.com/YOUR_USERNAME/zen-sync/releases/latest) page:

| Format | Notes |
|--------|-------|
| `.exe` (NSIS) | Recommended — standard Windows installer |
| `.msi` | For enterprise / group policy deployments |

> Windows may show a SmartScreen prompt on first run for unsigned builds. Click **More info → Run anyway** to proceed.

---

## Security

- Profile bundles are encrypted with **AES-256-GCM** before leaving your machine
- The encryption key is auto-generated on first connect and stored in **Windows Credential Manager** — it never touches disk in plaintext
- The backup repository is **private** and owned by your GitHub account
- A local timestamped backup (`zen-sync-backup-{timestamp}/`) is created inside your profile directory before every restore, giving you a manual rollback path independent of the GitHub history

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

## License

MIT — see [LICENSE](LICENSE).
