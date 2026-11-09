import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import LogViewer from "./components/LogViewer";
import { ConfirmProvider } from "./components/ConfirmDialog";
// Bundled with the app: nothing is fetched from the network to draw the UI.
import "@fontsource-variable/inter";
import "./index.css";

const isLogWindow = getCurrentWindow().label === "log";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {isLogWindow ? (
      <LogViewer />
    ) : (
      <ConfirmProvider>
        <App />
      </ConfirmProvider>
    )}
  </React.StrictMode>
);
