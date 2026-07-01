import { beforeEach, describe, expect, it, vi } from "vitest";
import type { UiState } from "../types";
import { openRecording, saveCurrentRecording } from "./rememberApi";

const tauriMocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
  open: vi.fn(),
  save: vi.fn()
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: tauriMocks.invoke
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: tauriMocks.listen
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: tauriMocks.open,
  save: tauriMocks.save
}));

const loadedState: UiState = {
  mode: "idle",
  recording_name: "loaded.remember.json",
  step_count: 3,
  duration_ms: 1200,
  message: "Loaded recording"
};

describe("rememberApi", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("returns null without invoking Rust when open dialog is cancelled", async () => {
    tauriMocks.open.mockResolvedValue(null);

    await expect(openRecording()).resolves.toBeNull();

    expect(tauriMocks.invoke).not.toHaveBeenCalled();
  });

  it("opens the selected recording path and returns the UI state", async () => {
    const path = "C:\\Recordings\\loaded.remember.json";
    tauriMocks.open.mockResolvedValue(path);
    tauriMocks.invoke.mockResolvedValue(loadedState);

    await expect(openRecording()).resolves.toBe(loadedState);

    expect(tauriMocks.open).toHaveBeenCalledWith({
      multiple: false,
      filters: [{ name: "Remember 录制文件", extensions: ["remember.json", "json"] }]
    });
    expect(tauriMocks.invoke).toHaveBeenCalledWith("open_recording", { path });
  });

  it("returns without invoking Rust when save dialog is cancelled", async () => {
    tauriMocks.save.mockResolvedValue(null);

    await expect(saveCurrentRecording()).resolves.toBeUndefined();

    expect(tauriMocks.invoke).not.toHaveBeenCalled();
  });

  it("saves the current recording to the selected path", async () => {
    const path = "C:\\Recordings\\current.remember.json";
    tauriMocks.save.mockResolvedValue(path);

    await saveCurrentRecording();

    expect(tauriMocks.save).toHaveBeenCalledWith({
      filters: [{ name: "Remember 录制文件", extensions: ["remember.json", "json"] }]
    });
    expect(tauriMocks.invoke).toHaveBeenCalledWith("save_current_recording", { path });
  });
});
