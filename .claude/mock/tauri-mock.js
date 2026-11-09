// Stand-in for the Tauri backend so the frontend can run in a plain browser.
// Only loaded through .claude/mock/vite.mock.config.mjs — never in real builds.
// Scenario knobs live on window.__mock and persist in sessionStorage.
(() => {
  const saved = JSON.parse(sessionStorage.getItem("mock") || "{}");
  const hoursAgo = (h) => new Date(Date.now() - h * 3600e3).toISOString();

  const m = (window.__mock = {
    connected: true,
    zenRunning: false,
    noSnapshots: false,
    manyPrefs: true,
    noExtensions: false,
    legacyCount: 0,
    delay: 300,
    fail: {}, // command name -> error message
    windowLabel: "main",
    ...saved,
    save() {
      const { save, emit, ...rest } = m;
      sessionStorage.setItem("mock", JSON.stringify(rest));
    },
    emit(event, payload) {
      for (const l of listeners) if (l.event === event) l.fn({ event, id: l.id, payload });
    },
  });

  const status = {
    connected: m.connected,
    username: "octocat",
    machineName: "Work Laptop",
    machineId: "a",
    lastBackupAt: m.noSnapshots ? null : hoursAgo(26),
    snapshotCount: 3,
    autostartEnabled: false,
    syncOptions: {
      sineMods: true,
      modSettings: true,
      extensionStorage: true,
      extensionPermissions: true,
      extensionShortcuts: true,
      zenShortcuts: true,
      aboutConfig: false,
    },
  };

  const snapshots = () =>
    m.noSnapshots
      ? []
      : [
          { index: 0, pushedAt: hoursAgo(3), machineName: "Home Desktop With A Really Long Device Name", sizeMb: 12.4, isCurrent: true, machineId: "b" },
          { index: 0, pushedAt: hoursAgo(26), machineName: "Work Laptop", sizeMb: 11.9, isCurrent: true, machineId: "a" },
          { index: 1, pushedAt: hoursAgo(80), machineName: "Work Laptop", sizeMb: 11.2, isCurrent: false, machineId: "a" },
          { index: 1, pushedAt: hoursAgo(200), machineName: "Home Desktop With A Really Long Device Name", sizeMb: 10.0, isCurrent: false, machineId: "b" },
          { index: 2, pushedAt: hoursAgo(400), machineName: "Work Laptop", sizeMb: 9.7, isCurrent: false, machineId: "a" },
        ];

  const extensions = [
    { id: "ublock@x", name: "uBlock Origin", version: "1.60.0", enabled: true, iconUrl: null, synced: true, storageBytes: 8_400_000, passwordManager: false },
    { id: "bitwarden@x", name: "Bitwarden Password Manager", version: "2026.10.1", enabled: true, iconUrl: null, synced: false, storageBytes: 2_100_000, passwordManager: true },
    { id: "dark@x", name: "Dark Reader", version: "4.9.99", enabled: true, iconUrl: null, synced: true, storageBytes: 310_000, passwordManager: false },
    { id: "sponsor@x", name: "SponsorBlock for YouTube - Skip Sponsorships", version: "5.9.4", enabled: false, iconUrl: null, synced: true, storageBytes: 0, passwordManager: false },
    { id: "stylus@x", name: "Stylus", version: "2.3.1", enabled: true, iconUrl: null, synced: true, storageBytes: 54_000, passwordManager: false },
    { id: "vim@x", name: "Vimium", version: "2.1.2", enabled: true, iconUrl: null, synced: true, storageBytes: 900, passwordManager: false },
    { id: "ref@x", name: "Refined GitHub", version: "25.1.1", enabled: true, iconUrl: null, synced: true, storageBytes: 12_000, passwordManager: false },
  ];

  const prefs = [];
  const names = ["browser.tabs.closeWindowWithLastTab", "zen.view.compact.hide-toolbar", "zen.theme.accent-color", "browser.urlbar.suggest.searches", "toolkit.legacyUserProfileCustomizations.stylesheets", "zen.workspaces.force-container-workspace", "browser.newtabpage.activity-stream.feeds.section.topstories", "layout.css.prefers-color-scheme.content-override"];
  for (let i = 0; i < (m.manyPrefs ? 140 : 8); i++) {
    const n = names[i % names.length] + (i >= names.length ? `.${i}` : "");
    prefs.push({ name: n, value: i % 3 === 0 ? "true" : i % 3 === 1 ? '"#7c6af7 with a rather long string value that keeps going and going beyond the row"' : "42", synced: i % 4 !== 0 });
  }

  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  const listeners = [];
  let nextId = 1;
  const callbacks = {};

  async function handle(cmd, args) {
    if (cmd === "plugin:event|listen") {
      const id = nextId++;
      listeners.push({ id, event: args.event, fn: callbacks[args.handler] });
      return id;
    }
    if (cmd === "plugin:event|unlisten") {
      const i = listeners.findIndex((l) => l.id === args.eventId);
      if (i >= 0) listeners.splice(i, 1);
      return;
    }
    if (cmd === "plugin:app|version") return "0.4.0";
    if (cmd.startsWith("plugin:")) return;
    if (cmd !== "is_zen_running") await wait(m.delay);
    if (m.fail[cmd]) throw m.fail[cmd];

    switch (cmd) {
      case "get_status_cmd":
        return { ...status, connected: m.connected, syncOptions: { ...status.syncOptions } };
      case "connect_github_cmd":
        await wait(1200);
        m.connected = true;
        m.save();
        return { ...status, connected: true };
      case "disconnect_github_cmd":
        m.connected = false;
        m.save();
        return;
      case "is_zen_running":
        return m.zenRunning;
      case "get_legacy_snapshot_count_cmd":
        return m.legacyCount;
      case "get_backup_summary_cmd":
        return { sineInstalled: true, sineEngineVersion: "2.1.4", modCount: 14, modSettingCount: 87, extensionCount: 6, storageBytes: 10_876_900, shortcutCount: 23, prefCount: prefs.filter((p) => p.synced).length };
      case "get_snapshots_cmd":
        return snapshots();
      case "backup_now_cmd":
        for (const step of ["Collecting mods…", "Encrypting…", "Uploading to GitHub…"]) {
          m.emit("sync-progress", step);
          await wait(900);
        }
        status.lastBackupAt = new Date().toISOString();
        return;
      case "restore_snapshot_cmd":
        for (const step of ["Downloading snapshot…", "Decrypting…", "Writing files…"]) {
          m.emit("sync-progress", step);
          await wait(900);
        }
        return { modCount: 14, extensionCount: 6, prefCount: 31, shortcutsRestored: true, warnings: m.restoreWarnings ? ["Dark Reader is not installed on this device, so its data was skipped.", "Sine is not installed; mods were restored but won't load until it is."] : [] };
      case "set_machine_name_cmd":
        status.machineName = args.name;
        return;
      case "set_snapshot_count_cmd":
        status.snapshotCount = args.count;
        return;
      case "set_sync_options_cmd":
        status.syncOptions = args.options;
        return;
      case "set_autostart_cmd":
        status.autostartEnabled = args.enabled;
        return;
      case "get_extensions_with_selection_cmd":
        return m.noExtensions ? [] : extensions;
      case "set_extension_selection_cmd":
        for (const e of extensions) e.synced = args.ids.includes(e.id);
        return;
      case "get_prefs_with_selection_cmd":
        return prefs;
      case "set_pref_selection_cmd":
        for (const p of prefs) p.synced = args.names.includes(p.name);
        return;
      case "get_log_cmd":
        return "2026-11-09 10:02:11 [INFO] backup started\n2026-11-09 10:02:14 [INFO] uploaded zen-sync-a-0.bin (11.9 MB)\n";
      default:
        console.warn("[mock] unhandled command", cmd, args);
    }
  }

  window.__TAURI_INTERNALS__ = {
    metadata: {
      currentWindow: { label: m.windowLabel },
      currentWebview: { label: m.windowLabel },
    },
    transformCallback(fn) {
      const id = nextId++;
      callbacks[id] = fn;
      return id;
    },
    invoke: (cmd, args = {}) => handle(cmd, args),
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
})();
