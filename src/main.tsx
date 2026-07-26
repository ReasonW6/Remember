import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ActivityIndicator } from "./ActivityIndicator";
import { AdvancedSettings } from "./AdvancedSettings";
import { App } from "./App";

const windowLabel = getCurrentWindow().label;
const isActivityIndicator = windowLabel === "activity-indicator";
if (isActivityIndicator) {
  document.documentElement.classList.add("activity-indicator-page");
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {isActivityIndicator ? (
      <ActivityIndicator />
    ) : windowLabel === "advanced-settings" ? (
      <AdvancedSettings />
    ) : (
      <App />
    )}
  </React.StrictMode>
);
