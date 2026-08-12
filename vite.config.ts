import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import { resolve } from "node:path";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        advancedSettings: resolve(__dirname, "advanced-settings.html"),
        activityIndicator: resolve(__dirname, "activity-indicator.html"),
        captureWarning: resolve(__dirname, "capture-warning.html"),
        windowBinding: resolve(__dirname, "window-binding.html"),
        windowHighlight: resolve(__dirname, "window-highlight.html")
      }
    }
  },
  server: {
    port: 1420,
    strictPort: true
  }
});
