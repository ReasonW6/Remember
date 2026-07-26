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
  renameRecording,
  setPlaybackSettings,
  startPlayback,
  confirmDeleteRecording,
  getAdvancedSettings,
  getPrivilegeState,
  restartAsAdministrator,
  setAdvancedSettings,
  showAdvancedSettings,
  subscribeToAdvancedSettingsChanged,
  subscribeToHotkeysChanged,
  subscribeToRecordingsChanged
} from "./rememberApi";

const tauriMocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
  open: vi.fn(),
  save: vi.fn(),
  ask: vi.fn()
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: tauriMocks.invoke
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: tauriMocks.listen
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: tauriMocks.open,
  save: tauriMocks.save,
  ask: tauriMocks.ask
}));

const loadedState: UiState = {
  mode: "idle",
  recording_name: "loaded.remember.json",
  step_count: 3,
  duration_ms: 1200,
  message: "Loaded recording",
  revision: 1,
  message_is_error: false
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

  it("renames a saved recording", async () => {
    const path = "C:\\Recordings\\selected.remember.json";
    const renamedPath = "C:\\Recordings\\weekly-report.remember.json";
    tauriMocks.invoke.mockResolvedValue(renamedPath);

    await expect(renameRecording(path, "weekly report")).resolves.toBe(renamedPath);

    expect(tauriMocks.invoke).toHaveBeenCalledWith("rename_recording", {
      path,
      newName: "weekly report"
    });
  });

  it("saves playback settings for hotkey playback", async () => {
    tauriMocks.invoke.mockResolvedValue(undefined);

    await expect(setPlaybackSettings(3, 2)).resolves.toBeUndefined();

    expect(tauriMocks.invoke).toHaveBeenCalledWith("set_playback_settings", {
      loopCount: 3,
      speedMultiplier: 2
    });
  });

  it("uses null loop counts for infinite playback", async () => {
    tauriMocks.invoke.mockResolvedValue(undefined);

    await expect(setPlaybackSettings(null, 2)).resolves.toBeUndefined();
    await expect(startPlayback(null, 2)).resolves.toBeUndefined();

    expect(tauriMocks.invoke).toHaveBeenNthCalledWith(1, "set_playback_settings", {
      loopCount: null,
      speedMultiplier: 2
    });
    expect(tauriMocks.invoke).toHaveBeenNthCalledWith(2, "start_playback", {
      loopCount: null,
      speedMultiplier: 2
    });
  });

  it("asks before permanently deleting a recording", async () => {
    tauriMocks.ask.mockResolvedValue(true);

    await expect(confirmDeleteRecording("demo-auto")).resolves.toBe(true);

    expect(tauriMocks.ask).toHaveBeenCalledWith(
      "确定要永久删除录制“demo-auto”吗？",
      expect.objectContaining({ title: "删除录制", kind: "warning" })
    );
  });

  it("subscribes to recording library changes", async () => {
    const callback = vi.fn();
    const unlisten = vi.fn();
    tauriMocks.listen.mockImplementation(async (_eventName, handler) => {
      handler({ payload: null });
      return unlisten;
    });

    await expect(subscribeToRecordingsChanged(callback)).resolves.toBe(unlisten);

    expect(tauriMocks.listen).toHaveBeenCalledWith(
      "remember://recordings-changed",
      expect.any(Function)
    );
    expect(callback).toHaveBeenCalledTimes(1);
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

  it("reads and saves advanced settings", async () => {
    const settings = {
      feedback_volume_percent: 75,
      feedback_muted: true,
      show_activity_indicator: false
    };
    tauriMocks.invoke.mockResolvedValue(settings);

    await expect(getAdvancedSettings()).resolves.toBe(settings);
    await expect(setAdvancedSettings(settings)).resolves.toBe(settings);

    expect(tauriMocks.invoke).toHaveBeenNthCalledWith(1, "get_advanced_settings");
    expect(tauriMocks.invoke).toHaveBeenNthCalledWith(2, "set_advanced_settings", { settings });
  });

  it("opens advanced settings and invokes administrator restart commands", async () => {
    tauriMocks.invoke
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce({ is_elevated: false })
      .mockResolvedValueOnce(undefined);

    await expect(showAdvancedSettings()).resolves.toBeUndefined();
    await expect(getPrivilegeState()).resolves.toEqual({ is_elevated: false });
    await expect(restartAsAdministrator()).resolves.toBeUndefined();

    expect(tauriMocks.invoke).toHaveBeenNthCalledWith(1, "show_advanced_settings");
    expect(tauriMocks.invoke).toHaveBeenNthCalledWith(2, "get_privilege_state");
    expect(tauriMocks.invoke).toHaveBeenNthCalledWith(3, "restart_as_administrator");
  });

  it("forwards hotkey and advanced-settings change events", async () => {
    const hotkeyCallback = vi.fn();
    const settingsCallback = vi.fn();
    const hotkeys = { record: "F9", playback: "F12", stop: "F9" };
    const settings = {
      feedback_volume_percent: 80,
      feedback_muted: false,
      show_activity_indicator: true
    };
    tauriMocks.listen.mockImplementation(async (eventName, handler) => {
      handler({ payload: eventName.includes("hotkeys") ? hotkeys : settings });
      return vi.fn();
    });

    await subscribeToHotkeysChanged(hotkeyCallback);
    await subscribeToAdvancedSettingsChanged(settingsCallback);

    expect(hotkeyCallback).toHaveBeenCalledWith(hotkeys);
    expect(settingsCallback).toHaveBeenCalledWith(settings);
  });
});
