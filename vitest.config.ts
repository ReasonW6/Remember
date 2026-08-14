import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    setupFiles: ["src/test/setup.ts"],
    globals: true,
    coverage: {
      provider: "v8",
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        "src/**/*.test.{ts,tsx}",
        "src/test/**",
        "src/**/*-main.tsx",
        "src/main.tsx",
        "src/types.ts",
        "src/vite-env.d.ts"
      ],
      thresholds: {
        statements: 85,
        branches: 75,
        functions: 75,
        lines: 85
      }
    }
  }
});
