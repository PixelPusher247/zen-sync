import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import LogViewer from "./components/LogViewer";
import "./index.css";

const isLogWindow = getCurrentWindow().label === "log";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {isLogWindow ? <LogViewer /> : <App />}
  </React.StrictMode>
);
