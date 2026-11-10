import { useState, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { bridge } from "./bridge";
import type { AppStatus, UpdateInfo, Screen } from "./types";
import SetupScreen from "./screens/SetupScreen";
import DashboardScreen from "./screens/DashboardScreen";
import SnapshotsScreen from "./screens/SnapshotsScreen";
import SettingsScreen from "./screens/SettingsScreen";
import ExtensionsScreen from "./screens/ExtensionsScreen";
import PrefsScreen from "./screens/PrefsScreen";
import KeyScreen from "./screens/KeyScreen";
import EncryptionScreen from "./screens/EncryptionScreen";
import UpdateBanner from "./components/UpdateBanner";
import NavBar from "./components/NavBar";
import OperationStatus from "./components/OperationStatus";
import { FloatingTitleBar } from "./components/WindowControls";
import { OperationsProvider } from "./hooks/useOperations";

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
    // The backend may finish restoring the saved sign-in before this
    // listener is attached, so check again once it is.
    const unlistenRestored = listen("github-restored", () => refreshStatus());
    unlistenRestored.then(() => refreshStatus());
    return () => {
      unlisten.then((f) => f());
      unlistenRestored.then((f) => f());
    };
  }, [refreshStatus]);

  // While a saved sign-in is still being restored, keep checking: covers a
  // missed event and shows the dashboard as soon as GitHub is reachable.
  const reconnecting = status?.reconnecting === true;
  useEffect(() => {
    if (!reconnecting) return;
    const id = setInterval(refreshStatus, 3000);
    return () => clearInterval(id);
  }, [reconnecting, refreshStatus]);

  if (loading) {
    return (
      <div className="relative h-full flex items-center justify-center bg-surface">
        <FloatingTitleBar />
        <div
          role="status"
          aria-label="Loading"
          className="w-6 h-6 border-2 border-accent border-t-transparent rounded-full animate-spin"
        />
      </div>
    );
  }

  if (!status?.connected) {
    return (
      <SetupScreen
        reconnecting={reconnecting}
        onConnected={(s) => {
          setStatus(s);
          setScreen("dashboard");
        }}
      />
    );
  }

  // Connected, but this device can't encrypt or decrypt snapshots yet.
  if (status.encryption !== "ready") {
    return <KeyScreen status={status} onStatusChange={setStatus} />;
  }

  // These screens bring their own header, with the close button in it.
  const isFullScreen = screen === "extensions" || screen === "prefs" || screen === "encryption";

  return (
    <OperationsProvider status={status} onStatusChange={setStatus}>
      <div className="h-full flex flex-col bg-surface animate-fade-in">
        {/* Header first so the close button stays in the window corner. */}
        {!isFullScreen && <NavBar screen={screen} onNavigate={setScreen} />}
        {update && !isFullScreen && (
          <UpdateBanner info={update} onDismiss={() => setUpdate(null)} />
        )}
        {!isFullScreen && <OperationStatus />}
        <main className="flex-1 min-h-0 overflow-y-auto">
          {screen === "dashboard" && (
            <DashboardScreen status={status} onNavigate={setScreen} />
          )}
          {screen === "snapshots" && <SnapshotsScreen status={status} />}
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
          {screen === "prefs" && (
            <PrefsScreen onBack={() => setScreen("settings")} />
          )}
          {screen === "encryption" && (
            <EncryptionScreen onBack={() => setScreen("settings")} />
          )}
        </main>
      </div>
    </OperationsProvider>
  );
}
