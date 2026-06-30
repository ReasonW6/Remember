import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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

export async function subscribeToState(onState: (state: UiState) => void) {
  return listen<UiState>("remember://state", (event) => {
    onState(event.payload);
  });
}
