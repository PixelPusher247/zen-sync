# Zen Sync — Setup Guide

## Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- Node.js 20+
- Windows 10/11

## 1. Register a GitHub OAuth App

1. Go to **GitHub → Settings → Developer settings → OAuth Apps → New OAuth App**
2. Fill in:
   - **Application name**: `Zen Sync`
   - **Homepage URL**: `https://github.com/YOUR_USERNAME/zen-sync`
   - **Authorization callback URL**: `http://127.0.0.1`
3. After creating, copy the **Client ID** and generate a **Client secret**

## 2. Configure environment

Copy `.env.example` to `.env` and fill in your credentials:

```
GITHUB_CLIENT_ID=your_client_id_here
GITHUB_CLIENT_SECRET=your_client_secret_here
```

For building you must export these before running `npm run build`:

```powershell
$env:GITHUB_CLIENT_ID = "your_client_id"
$env:GITHUB_CLIENT_SECRET = "your_client_secret"
```

## 3. Generate an updater signing key

```powershell
npm run tauri signer generate -- -w ~/.tauri/zen-sync.key
```

Copy the public key into `src-tauri/tauri.conf.json` under `plugins.updater.pubkey`.  
Store the private key as the `TAURI_SIGNING_PRIVATE_KEY` secret in GitHub repository settings.

## 4. Update the updater endpoint

In `src-tauri/tauri.conf.json`, replace `PLACEHOLDER_GITHUB_USER` with your GitHub username:

```json
"endpoints": [
  "https://github.com/YOUR_USERNAME/zen-sync/releases/latest/download/latest.json"
]
```

## 5. Install dependencies and run

```powershell
npm install
npm run dev        # development mode with hot-reload
npm run build      # production build (Windows .msi + .exe)
```

## 6. Releasing

Push a tag to trigger the release workflow:

```powershell
git tag v0.1.0
git push origin v0.1.0
```

GitHub Actions will build the Windows installer and publish a GitHub Release with `latest.json` for the auto-updater.  
Required repository secrets:
- `OAUTH_CLIENT_ID`
- `OAUTH_CLIENT_SECRET`
- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

## What gets synced

| File | Purpose |
|------|---------|
| `places.sqlite` | Bookmarks, pinned tabs, workspaces |
| `prefs.js` | Browser preferences (**Firefox Sync settings and device name are excluded**) |
| `extensions.json` | Installed extension list |
| `zen-themes.json` | Mods/themes configuration |
| `zen-keyboard-shortcuts.json` | Keyboard shortcuts |
| `zen-sessions.jsonlz4` | Workspace names, tab assignments |
| `zen-live-folders.jsonlz4` | Live folders |
| `chrome/zen-themes.css` | Compiled mod styles |
| `containers.json` | Workspace icons/colors |

## How it differs from Zync

| Issue in Zync | Fix in Zen Sync |
|---|---|
| Browser stuck in weird state after restore | Hard block: refuses backup/restore if Zen is running |
| Device name overwritten on restore | `prefs.js` filtered: `services.sync.*`, `identity.fxaccounts.*`, and other per-machine keys are stripped before upload and re-injected after restore |
| Auto-sync happening without user action | No background daemons — 100% manual |
| Autostart enabled by default | Off by default; toggle in tray menu or Settings |
