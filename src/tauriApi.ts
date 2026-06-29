import { invoke } from "@tauri-apps/api/core";
import type {
  EmergencyHotkeyStatusPayload,
  Flow,
  FlowSummary,
  PlaybackControlPayload,
  PlaybackStartPayload,
  RecordingStartPayload,
  RecordingStopPayload,
  SavedFlow,
} from "./types";

export const EMERGENCY_HOTKEY_SHORTCUT = "Ctrl + Alt + S";

function isTauriRuntime() {
  return "__TAURI_INTERNALS__" in window;
}

function browserPreviewUnavailable(error: unknown): never {
  if (isTauriRuntime()) throw error;
  throw new Error("Remember must run inside the Tauri desktop app.");
}

async function invokeDesktop<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    browserPreviewUnavailable(error);
  }
}

export async function getInitialFlow(): Promise<SavedFlow> {
  return invokeDesktop<SavedFlow>("get_initial_flow");
}

export async function listFlows(): Promise<FlowSummary[]> {
  return invokeDesktop<FlowSummary[]>("list_flows");
}

export async function loadFlow(fileName: string): Promise<SavedFlow> {
  return invokeDesktop<SavedFlow>("load_flow", { fileName });
}

export async function getEmergencyHotkeyStatus(): Promise<EmergencyHotkeyStatusPayload> {
  return invokeDesktop<EmergencyHotkeyStatusPayload>(
    "get_emergency_hotkey_status",
  );
}

export async function saveFlow(
  fileName: string,
  flow: Flow,
): Promise<SavedFlow> {
  return invokeDesktop<SavedFlow>("save_flow", { fileName, flow });
}

export async function saveFlowAs(
  flow: Flow,
  displayName: string,
): Promise<SavedFlow> {
  return invokeDesktop<SavedFlow>("save_flow_as", { flow, displayName });
}

export async function startRecording(): Promise<RecordingStartPayload> {
  return invokeDesktop<RecordingStartPayload>("start_recording");
}

export async function stopRecording(): Promise<RecordingStopPayload> {
  return invokeDesktop<RecordingStopPayload>("stop_recording");
}

export async function openWorkbench(): Promise<void> {
  await invokeDesktop<void>("show_workbench");
}

export async function startPlayback(
  flow: Flow,
  speedMultiplier: number,
  loopCount: number,
  infiniteLoopConfirmed = false,
): Promise<PlaybackStartPayload> {
  return invokeDesktop<PlaybackStartPayload>("start_playback", {
    flow,
    speedMultiplier,
    loopCount,
    infiniteLoopConfirmed,
  });
}

export async function stopPlayback(): Promise<PlaybackControlPayload> {
  return invokeDesktop<PlaybackControlPayload>("stop_playback");
}

export async function emergencyStopPlayback(): Promise<PlaybackControlPayload> {
  return invokeDesktop<PlaybackControlPayload>("emergency_stop_playback");
}
