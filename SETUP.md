# Zen Sync — Setup Guide

## Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust (stable) | 1.77+ | [rustup.rs](https://rustup.rs/) |
| Node.js | 20+ | [nodejs.org](https://nodejs.org/) |
| Windows | 10 / 11 | — |

After installing Rust, verify it works:

```powershell
rustc --version   # e.g. rustc 1.77.0
cargo --version   # e.g. cargo 1.77.0
```

---

## Step 1 — Fork / clone the repo

```powershell
git clone https://github.com/PixelPusher247/zen-sync.git
cd zen-sync
npm install
```

---

## Step 2 — Register a GitHub OAuth App

Zen Sync uses GitHub OAuth so users can connect their GitHub account without
sharing a password.  You register one OAuth App per deployment (your fork).

1. Go to [github.com/settings/developers](https://github.com/settings/developers)
2. Click **New OAuth App**
3. Fill in:

   | Field | Value |
   |-------|-------|
   | Application name | `Zen Sync` |
   | Homepage URL | `https://github.com/PixelPusher247/zen-sync` |
   | Authorization callback URL | `http://127.0.0.1` |

4. Click **Register application**
5. On the next page, note the **Client ID**
6. Click **Generate a new client secret** and copy the secret immediately

---

## Step 3 — Set environment variables for local builds

The client ID and secret are compiled into the binary via `env!()` macros,
so they must be set as environment variables **before** every build.

Create a `.env.local` file (gitignored):

```
GITHUB_CLIENT_ID=Ov23liXXXXXXXXXXXXXX
GITHUB_CLIENT_SECRET=xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
```

Then load it before building:

```powershell
# Load from .env.local (run once per terminal session)
Get-Content .env.local | ForEach-Object {
    if ($_ -match '^([^#][^=]+)=(.+)$') {
        [System.Environment]::SetEnvironmentVariable($matches[1].Trim(), $matches[2].Trim(), 'Process')
    }
}

# Verify they are set
echo $env:GITHUB_CLIENT_ID
echo $env:GITHUB_CLIENT_SECRET
```

---

## Step 4 — Generate an updater signing key

Tauri's auto-updater requires a signing key pair.  The public key goes in the
config; the private key signs release bundles in CI.

```powershell
# Generate the key pair (only needed once)
npm run tauri -- signer generate -w "$env:USERPROFILE\.tauri\zen-sync.key"
```

This prints something like:

```
Public key: dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXkgAAAA...
```

**Copy the public key** and paste it into `src-tauri/tauri.conf.json`:

```json
"plugins": {
  "updater": {
    "pubkey": "PASTE_PUBLIC_KEY_HERE",
    "endpoints": [
      "https://github.com/PixelPusher247/zen-sync/releases/latest/download/latest.json"
    ]
  }
}
```

Also replace `PixelPusher247` in the endpoint URL with your GitHub username.

---

## Step 5 — Add GitHub repository secrets

Go to your fork on GitHub → **Settings → Secrets and variables → Actions**
and add these repository secrets:

| Secret name | Value |
|-------------|-------|
| `OAUTH_CLIENT_ID` | Client ID from Step 2 |
| `OAUTH_CLIENT_SECRET` | Client secret from Step 2 |
| `TAURI_SIGNING_PRIVATE_KEY` | Contents of `~/.tauri/zen-sync.key` |

> **Tip:** To read the private key file: `Get-Content "$env:USERPROFILE\.tauri\zen-sync.key"`
>
> If you did not set a password when generating the key, no `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` secret is needed — the workflow omits it and Tauri defaults to no password.

---

## Step 6 — Run locally

```powershell
# Start the app in development mode (hot-reload)
npm run dev

# Build the installers and the portable exe
npm run build
# Output: src-tauri/target/release/bundle/nsis/*.exe
#         src-tauri/target/release/bundle/msi/*.msi
#         src-tauri/target/release/bundle/portable/*-portable.exe

# Build only the portable exe (no installer; update banner links to the download page)
npm run build:portable
```

Installer builds also sign the updater artifacts, so set
`TAURI_SIGNING_PRIVATE_KEY` (see Step 4) before `npm run build`.
`npm run build:portable` doesn't need it.

---

## Step 7 — Release a new version

Commit your changes, then run the release script with the new version:

```powershell
.\release.ps1 -Version 0.2.1
```

It bumps the version in `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml`,
commits, tags `v0.2.1`, and pushes `main` and the tag.

The `release.yml` workflow runs automatically and:
- Builds the Windows NSIS installer and MSI
- Creates a GitHub Release with both installers attached
- Uploads `latest.json` so existing installs can auto-update
- Builds the portable exe and attaches it as `Zen.Sync_<version>_x64-portable.exe`

Users running an older version will see an update banner in the app within
24 hours. Installed copies update in place; the portable exe opens the
release page so the new exe can be downloaded.

---

## What gets synced

Zen's Mozilla account sync covers spaces, containers, bookmarks, history,
passwords, installed extensions and `storage.sync`. Zen Sync only backs up
what it misses. A snapshot is an encrypted zip with a `manifest.json`:

| Data | Source | Notes |
|------|--------|-------|
| Sine mods | `chrome/sine-mods/` | Mirrored on restore. Sine's generated `chrome.css` / `content.css` are skipped; Sine rebuilds them on startup |
| Mod settings | `prefs.js` | Only prefs declared as `property` in a mod's `preferences.json` or Sine's `chrome/JS/core/settings.json`; settings missing from the snapshot are reset to default |
| Extension storage | `storage/default/moz-extension+++<uuid>^userContextId=4294967295/` | `storage.local`; see below |
| Extension permissions | `extension-preferences.json` | This extension's entry only |
| Extension shortcuts | `extension-settings.json` → `commands` | This extension's entries only; the local install date is kept |

No other `prefs.js` line is read or written, so the Mozilla account binding and
device name are never touched. Prefs such as `services.sync.*`, `identity.*`
and `app.update.*` are protected even if a mod declares them.

### Extension storage and UUIDs

Firefox gives each extension a random UUID per profile
(`extensions.webextensions.uuids`). The UUID is part of the storage folder name
and is also stored inside the folder's `.metadata-v2` file and the IndexedDB
`database.origin` column. On restore, Zen Sync:

- uses the UUID the extension already has on this device, rewriting both
  places, or adds the snapshot's UUID to the map if the extension isn't
  installed yet
- sets `extensions.webextensions.ExtensionStorageIDB.migrated.<id>`
- marks the QuotaManager cache in `storage.sqlite` invalid so Zen rescans the
  storage folders on next start

### Per-extension selection

In **Settings → Extensions to back up** you can toggle individual extensions.
Extensions are included by default, except password managers (Bitwarden,
1Password, LastPass, KeePassXC, or anything named like one): their local
storage holds account and device session state that shouldn't move between
machines. Only choices that differ from the default are saved.

### Safety copies

Before a restore, every file and folder it will replace is copied to
`%APPDATA%\app.zen.zensync\restore-backups\<timestamp>\`. The last three copies
are kept.

---

## How it differs from Zync

| Issue in Zync | Fix in Zen Sync |
|---|---|
| Browser stuck in weird state after restore | Hard block: refuses all operations while Zen is running |
| Device name overwritten on restore | Only mod settings are written to `prefs.js`; account and device prefs are never touched |
| Auto-sync happening without user action | No background daemons — 100% manual trigger |
| Autostart enabled by default | Off by default; toggle in tray or Settings |
| All extensions always synced | Per-extension toggle in Settings → Extensions to back up |

---

## Troubleshooting

**Build fails: `environment variable GITHUB_CLIENT_ID not set`**
You forgot to export the env vars before running the build — see Step 3.

**`cargo` not found after installing Rust**
Restart your terminal so `%USERPROFILE%\.cargo\bin` is on `PATH`, or run:
```powershell
$env:PATH += ";$env:USERPROFILE\.cargo\bin"
```

**OAuth redirect doesn't come back to the app**
Make sure the Authorization callback URL in your GitHub OAuth App is exactly
`http://127.0.0.1` (no trailing slash, no port number).

**App shows "No backup data found" after first launch**
This is normal — you need to perform a backup from at least one device before
the History screen has anything to show.

**Auto-updater shows "Update check failed"**
Check that `plugins.updater.endpoints` in `tauri.conf.json` points to your
fork's releases, and that you have published at least one release with a
`latest.json` asset.
