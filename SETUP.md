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
git clone https://github.com/YOUR_USERNAME/zen-sync.git
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
   | Homepage URL | `https://github.com/YOUR_USERNAME/zen-sync` |
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
      "https://github.com/YOUR_USERNAME/zen-sync/releases/latest/download/latest.json"
    ]
  }
}
```

Also replace `YOUR_USERNAME` in the endpoint URL with your GitHub username.

---

## Step 5 — Add GitHub repository secrets

Go to your fork on GitHub → **Settings → Secrets and variables → Actions**
and add these repository secrets:

| Secret name | Value |
|-------------|-------|
| `OAUTH_CLIENT_ID` | Client ID from Step 2 |
| `OAUTH_CLIENT_SECRET` | Client secret from Step 2 |
| `TAURI_SIGNING_PRIVATE_KEY` | Contents of `~/.tauri/zen-sync.key` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password you chose when generating the key (leave blank if none) |

> **Tip:** To read the private key file: `Get-Content "$env:USERPROFILE\.tauri\zen-sync.key"`

---

## Step 6 — Run locally

```powershell
# Start the app in development mode (hot-reload)
npm run dev

# Build a production installer
npm run build
# Output: src-tauri/target/release/bundle/nsis/*.exe
#         src-tauri/target/release/bundle/msi/*.msi
```

---

## Step 7 — Release a new version

1. Update the version in `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml`
2. Commit and push
3. Create and push a version tag:

```powershell
git tag v0.2.0
git push origin v0.2.0
```

The `release.yml` workflow runs automatically and:
- Builds the Windows NSIS installer and MSI
- Creates a GitHub Release with both installers attached
- Uploads `latest.json` so existing installs can auto-update

Users running an older version will see an "Install update" banner in the app
within 24 hours (or immediately from the tray → Check for updates).

---

## What gets synced

| File | Purpose | Notes |
|------|---------|-------|
| `places.sqlite` | Bookmarks, pinned tabs, workspaces | WAL-checkpointed before backup |
| `prefs.js` | Browser preferences | Per-machine keys stripped — see below |
| `extensions.json` | Installed extension list | Selectable per-extension — see below |
| `zen-themes.json` | Mods/themes configuration | |
| `zen-keyboard-shortcuts.json` | Keyboard shortcuts | |
| `zen-sessions.jsonlz4` | Workspace names, tab assignments | |
| `zen-live-folders.jsonlz4` | Live folders | |
| `chrome/zen-themes.css` | Compiled mod styles | |
| `containers.json` | Workspace icons/colors | |

### prefs.js — per-machine keys excluded

The following preference key prefixes are **stripped before upload** and
**re-injected from the local device after restore**, so they are never
overwritten:

- `services.sync.*` — Firefox Sync account binding and device name
- `identity.fxaccounts.*` — Firefox Accounts identity
- `identity.sync.*` — Sync server settings
- `app.update.*`, `zen.updates.*` — Per-device update state
- `toolkit.telemetry.cachedClientID` — Telemetry ID
- `browser.sessionstore.*` — Machine-specific session restore state
- `browser.startup.homepage_override.*` — Build-specific migration markers

### extensions.json — per-extension selection

In **Settings → Extensions to sync** you can toggle individual extensions
on or off.  The default (nothing explicitly selected) means **all** user
extensions are included.

When backing up, only the selected extension entries are written into the
encrypted bundle.  When restoring, only the selected extensions are merged
into the local `extensions.json` — all other extensions on the target device
are left untouched.

Extension storage data (the per-extension databases in `storage/`) is **not**
synced.  Extensions must re-sync their own data through their own mechanisms
(e.g. uBlock Origin's cloud backup, Bitwarden's vault sync).

---

## How it differs from Zync

| Issue in Zync | Fix in Zen Sync |
|---|---|
| Browser stuck in weird state after restore | Hard block: refuses all operations while Zen is running |
| Device name overwritten on restore | `prefs.js` key filtering preserves `services.sync.client.name` and all other per-machine keys |
| Auto-sync happening without user action | No background daemons — 100% manual trigger |
| Autostart enabled by default | Off by default; toggle in tray or Settings |
| All extensions always synced | Per-extension toggle in Settings → Extensions to sync |

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
