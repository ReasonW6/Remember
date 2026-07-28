import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { ComponentType } from "react";
import "./styles.css";

const windowLabel = getCurrentWindow().label;
const isActivityIndicator = windowLabel === "activity-indicator";
if (isActivityIndicator) {
  document.documentElement.classList.add("activity-indicator-page");
}

async function renderWindow() {
  let WindowComponent: ComponentType;

  if (isActivityIndicator) {
    WindowComponent = (await import("./ActivityIndicator")).ActivityIndicator;
  } else if (windowLabel === "advanced-settings") {
    WindowComponent = (await import("./AdvancedSettings")).AdvancedSettings;
  } else {
    WindowComponent = (await import("./App")).App;
  }

  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <WindowComponent />
    </React.StrictMode>
  );
}

void renderWindow();
