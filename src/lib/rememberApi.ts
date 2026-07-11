import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import type { HotkeyConfig, RecordingFile, UiState } from "../types";

export function getState() {
  return invoke<UiState>("get_state");
}

export function startRecording() {
  return invoke<UiState>("start_recording");
}

export function stopRecording() {
  return invoke<UiState>("stop_recording");
}

export function listRecordings() {
  return invoke<RecordingFile[]>("list_recordings");
}

export function deleteRecording(path: string) {
  return invoke<void>("delete_recording", { path });
}

export function renameRecording(path: string, newName: string) {
  return invoke<string>("rename_recording", { path, newName });
}

export function setPlaybackSettings(loopCount: number | null, speedMultiplier: number) {
  return invoke<void>("set_playback_settings", {
    loopCount,
    speedMultiplier
  });
}

export function startPlayback(loopCount: number | null, speedMultiplier: number) {
  return invoke<UiState>("start_playback", {
    loopCount,
    speedMultiplier
  });
}

export function stopPlayback() {
  return invoke<UiState>("stop_playback");
}

export async function openRecording(): Promise<UiState | null> {
  const selected = await open({
    multiple: false,
    filters: [{ name: "Remember 录制文件", extensions: ["remember.json", "json"] }]
  });

  if (typeof selected !== "string") {
    return null;
  }

  return invoke<UiState>("open_recording", { path: selected });
}

export function loadRecording(path: string) {
  return invoke<UiState>("open_recording", { path });
}

export async function saveCurrentRecording(): Promise<void> {
  const selected = await save({
    filters: [{ name: "Remember 录制文件", extensions: ["remember.json", "json"] }]
  });

  if (!selected) {
    return;
  }

  await invoke("save_current_recording", { path: selected });
}

export function getHotkeys() {
  return invoke<HotkeyConfig>("get_hotkeys");
}

export function setHotkeys(config: HotkeyConfig) {
  return invoke<HotkeyConfig>("set_hotkeys", { config });
}

export function confirmDeleteRecording(name: string) {
  return ask(`确定要永久删除录制“${name}”吗？`, {
    title: "删除录制",
    kind: "warning"
  });
}

export async function subscribeToState(onState: (state: UiState) => void) {
  return listen<UiState>("remember://state", (event) => {
    onState(event.payload);
  });
}

export async function subscribeToRecordingsChanged(onChanged: () => void) {
  return listen("remember://recordings-changed", () => {
    onChanged();
  });
}
