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
import { FloatingTitleBar } from "./components/WindowControls";

export default function App() {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [screen, setScreen] = useState<Screen>("dashboard");
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [loading, setLoading] = useState(true);

  const refreshStatus = useCallback(async () => {
    try {
      const s = await bridge.getStatus();
      setStatus(s);
      if (!s.connected) {
        setScreen("setup");
      } else {
        setScreen((prev) => (prev === "setup" ? "dashboard" : prev));
      }
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
      <div className="relative h-full flex items-center justify-center bg-surface">
        <FloatingTitleBar />
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
      {/* Header first so the close button stays in the window corner. */}
      {screen !== "extensions" && (
        <NavBar
          screen={screen}
          onNavigate={setScreen}
        />
      )}
      {update && screen !== "extensions" && (
        <UpdateBanner
          info={update}
          onDismiss={() => setUpdate(null)}
        />
      )}
      <main className="flex-1 overflow-y-auto">
        {screen === "dashboard" && (
          <DashboardScreen
            status={status}
            onStatusChange={setStatus}
            onNavigate={setScreen}
          />
        )}
        {screen === "snapshots" && (
          <SnapshotsScreen status={status} onStatusChange={setStatus} />
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
