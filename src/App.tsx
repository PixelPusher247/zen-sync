import { useState, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { bridge } from "./bridge";
import type { AppStatus, UpdateInfo, Screen } from "./types";
import SetupScreen from "./screens/SetupScreen";
import DashboardScreen from "./screens/DashboardScreen";
import SnapshotsScreen from "./screens/SnapshotsScreen";
import SettingsScreen from "./screens/SettingsScreen";
import ExtensionsScreen from "./screens/ExtensionsScreen";
import UpdateBanner from "./components/UpdateBanner";
import NavBar from "./components/NavBar";

export default function App() {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [screen, setScreen] = useState<Screen>("dashboard");
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [loading, setLoading] = useState(true);

  const refreshStatus = useCallback(async () => {
    try {
      const s = await bridge.getStatus();
      setStatus(s);
      if (!s.connected) setScreen("setup");
    } catch (e) {
      console.error("getStatus failed:", e);
    }
  }, []);

  useEffect(() => {
    refreshStatus().finally(() => setLoading(false));
  }, [refreshStatus]);

  useEffect(() => {
    const unlisten = listen<UpdateInfo>("update-available", (e) => {
      setUpdate(e.payload);
    });
    const unlistenRestored = listen("github-restored", () => refreshStatus());
    return () => {
      unlisten.then((f) => f());
      unlistenRestored.then((f) => f());
    };
  }, [refreshStatus]);

  if (loading) {
    return (
      <div className="h-full flex items-center justify-center bg-surface">
        <div className="w-6 h-6 border-2 border-accent border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  if (!status?.connected) {
    return (
      <SetupScreen
        onConnected={(s) => {
          setStatus(s);
          setScreen("dashboard");
        }}
      />
    );
  }

  return (
    <div className="h-full flex flex-col bg-surface animate-fade-in">
      {update && (
        <UpdateBanner
          info={update}
          onDismiss={() => setUpdate(null)}
        />
      )}
      {screen !== "extensions" && (
        <NavBar
          screen={screen}
          onNavigate={setScreen}
        />
      )}
      <main className="flex-1 overflow-y-auto">
        {screen === "dashboard" && (
          <DashboardScreen
            status={status}
            onStatusChange={setStatus}
          />
        )}
        {screen === "snapshots" && (
          <SnapshotsScreen onStatusChange={setStatus} />
        )}
        {screen === "settings" && (
          <SettingsScreen
            status={status}
            onStatusChange={setStatus}
            onNavigate={setScreen}
          />
        )}
        {screen === "extensions" && (
          <ExtensionsScreen onBack={() => setScreen("settings")} />
        )}
      </main>
    </div>
  );
}
