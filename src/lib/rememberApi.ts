import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { UiState } from "../types";

export function getState() {
  return invoke<UiState>("get_state");
}

export function startRecording() {
  return invoke<UiState>("start_recording");
}

export function stopRecording() {
  return invoke<UiState>("stop_recording");
}

export function startPlayback(loopCount: number, speedMultiplier: number) {
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

export async function saveCurrentRecording(): Promise<void> {
  const selected = await save({
    filters: [{ name: "Remember 录制文件", extensions: ["remember.json", "json"] }]
  });

  if (!selected) {
    return;
  }

  await invoke("save_current_recording", { path: selected });
}

export async function subscribeToState(onState: (state: UiState) => void) {
  return listen<UiState>("remember://state", (event) => {
    onState(event.payload);
  });
}
