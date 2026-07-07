import { beforeEach, describe, expect, it, vi } from "vitest";
import type { UiState } from "../types";
import {
  getHotkeys,
  listRecordings,
  loadRecording,
  openRecording,
  saveCurrentRecording,
  setHotkeys,
  deleteRecording,
  setPlaybackSettings
} from "./rememberApi";

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

  it("loads a recording directly by path", async () => {
    const path = "C:\\Recordings\\selected.remember.json";
    tauriMocks.invoke.mockResolvedValue(loadedState);

    await expect(loadRecording(path)).resolves.toBe(loadedState);

    expect(tauriMocks.invoke).toHaveBeenCalledWith("open_recording", { path });
  });

  it("lists saved recordings", async () => {
    const recordings = [
      {
        name: "selected",
        path: "C:\\Recordings\\selected.remember.json",
        step_count: 3,
        duration_ms: 1200,
        created_at: "2026-07-01T00:00:00Z",
        updated_at_ms: 1782864000000
      }
    ];
    tauriMocks.invoke.mockResolvedValue(recordings);

    await expect(listRecordings()).resolves.toBe(recordings);

    expect(tauriMocks.invoke).toHaveBeenCalledWith("list_recordings");
  });

  it("deletes a saved recording", async () => {
    const path = "C:\\Recordings\\selected.remember.json";
    tauriMocks.invoke.mockResolvedValue(undefined);

    await expect(deleteRecording(path)).resolves.toBeUndefined();

    expect(tauriMocks.invoke).toHaveBeenCalledWith("delete_recording", { path });
  });

  it("saves playback settings for hotkey playback", async () => {
    tauriMocks.invoke.mockResolvedValue(undefined);

    await expect(setPlaybackSettings(3, 2)).resolves.toBeUndefined();

    expect(tauriMocks.invoke).toHaveBeenCalledWith("set_playback_settings", {
      loopCount: 3,
      speedMultiplier: 2
    });
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

  it("reads and saves hotkeys", async () => {
    const config = { record: "F6", playback: "F7", stop: "F8" };
    tauriMocks.invoke.mockResolvedValue(config);

    await expect(getHotkeys()).resolves.toBe(config);
    await expect(setHotkeys(config)).resolves.toBe(config);

    expect(tauriMocks.invoke).toHaveBeenNthCalledWith(1, "get_hotkeys");
    expect(tauriMocks.invoke).toHaveBeenNthCalledWith(2, "set_hotkeys", { config });
  });
});
