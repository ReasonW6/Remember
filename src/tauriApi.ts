import { invoke } from "@tauri-apps/api/core";
import { sampleSavedFlow } from "./data/sampleFlow";
import type {
  AppStatus,
  AppStatusPayload,
  Flow,
  FlowSummary,
  PlaybackControlPayload,
  PlaybackStartPayload,
  RecordingStartPayload,
  RecordingStopPayload,
  SavedFlow,
} from "./types";

const statusCommand: Record<AppStatus, string> = {
  ready: "status_ready",
  recording: "status_recording",
  playing: "status_playing",
  stopped: "status_stopped",
};

export const RECORDING_SAFETY_WARNING =
  "录制会记录鼠标和键盘操作，请勿在录制期间输入密码、验证码或其他敏感信息。";

function isTauriRuntime() {
  return "__TAURI_INTERNALS__" in window;
}

export async function getInitialFlow(): Promise<SavedFlow> {
  try {
    return await invoke<SavedFlow>("get_initial_flow");
  } catch (error) {
    if (isTauriRuntime()) throw error;
    return sampleSavedFlow;
  }
}

export async function listFlows(): Promise<FlowSummary[]> {
  try {
    return await invoke<FlowSummary[]>("list_flows");
  } catch (error) {
    if (isTauriRuntime()) throw error;
    return [
      {
        fileName: sampleSavedFlow.fileName,
        name: sampleSavedFlow.flow.name,
        displayName: sampleSavedFlow.flow.displayName,
        stepCount: sampleSavedFlow.flow.steps.length,
        savedAt: sampleSavedFlow.savedAt,
      },
    ];
  }
}

export async function loadFlow(fileName: string): Promise<SavedFlow> {
  try {
    return await invoke<SavedFlow>("load_flow", { fileName });
  } catch (error) {
    if (isTauriRuntime()) throw error;
    return sampleSavedFlow;
  }
}

export async function saveFlow(flow: Flow): Promise<SavedFlow> {
  try {
    return await invoke<SavedFlow>("save_flow", { flow });
  } catch (error) {
    if (isTauriRuntime()) throw error;
    return {
      ...sampleSavedFlow,
      savedAt: Math.floor(Date.now() / 1000),
      flow,
    };
  }
}

export async function saveFlowAs(
  flow: Flow,
  displayName: string,
): Promise<SavedFlow> {
  try {
    return await invoke<SavedFlow>("save_flow_as", { flow, displayName });
  } catch (error) {
    if (isTauriRuntime()) throw error;
    const normalizedDisplayName = displayName.trim() || "Untitled Flow";
    const name = normalizedDisplayName
      .toLowerCase()
      .replace(/[^a-z0-9_-]+/g, "-")
      .replace(/^-+|-+$/g, "") || "untitled-flow";

    return {
      fileName: `${name}.remember.json`,
      savedAt: Math.floor(Date.now() / 1000),
      flow: {
        ...flow,
        name,
        displayName: normalizedDisplayName,
      },
    };
  }
}

export async function startRecording(): Promise<RecordingStartPayload> {
  try {
    return await invoke<RecordingStartPayload>("start_recording");
  } catch (error) {
    if (isTauriRuntime()) throw error;
    return {
      status: "recording",
      label: "录制中",
      startedAt: Math.floor(Date.now() / 1000),
      warning: "已启动录制会话；当前会捕获鼠标点击、键盘输入和热键。",
      safetyWarning: RECORDING_SAFETY_WARNING,
    };
  }
}

export async function stopRecording(): Promise<RecordingStopPayload> {
  const stoppedAt = Math.floor(Date.now() / 1000);
  try {
    return await invoke<RecordingStopPayload>("stop_recording");
  } catch (error) {
    if (isTauriRuntime()) throw error;
    return {
      status: "stopped",
      label: "已停止",
      startedAt: stoppedAt,
      stoppedAt,
      flow: {
        version: 1,
        name: `recording-${stoppedAt}`,
        displayName: `录制会话安全占位 ${stoppedAt}`,
        targetWindow: {
          title: "尚未捕获活动窗口",
          process: "N/A",
          size: "N/A",
          matched: false,
        },
        steps: [
          {
            id: 1,
            type: "wait",
            action: "等待",
            durationMs: 500,
            delayMs: 500,
            note: "安全占位步骤：尚未捕获真实输入",
          },
        ],
      },
      message: "已停止录制会话；生成的是安全占位流程，尚未捕获真实输入。",
    };
  }
}

export async function openWorkbench(): Promise<void> {
  try {
    await invoke("show_workbench");
  } catch (error) {
    if (isTauriRuntime()) throw error;
    window.location.hash = "#/workbench";
  }
}

export async function startPlayback(
  flow: Flow,
  speedMultiplier: number,
  loopCount: number,
  infiniteLoopConfirmed = false,
): Promise<PlaybackStartPayload> {
  try {
    return await invoke<PlaybackStartPayload>("start_playback", {
      flow,
      speedMultiplier,
      loopCount,
      infiniteLoopConfirmed,
    });
  } catch (error) {
    if (isTauriRuntime()) throw error;
    return {
      runId: Date.now(),
      status: "playing",
      label: "回放中",
      flowName: flow.displayName,
      loopCount,
      speedMultiplier,
      message: "开始回放；浏览器预览不会执行本地自动化。",
    };
  }
}

export async function stopPlayback(): Promise<PlaybackControlPayload> {
  try {
    return await invoke<PlaybackControlPayload>("stop_playback");
  } catch (error) {
    if (isTauriRuntime()) throw error;
    return {
      status: "stopped",
      label: "已停止",
      reason: "stopped",
      message: "已请求停止当前回放。",
    };
  }
}

export async function emergencyStopPlayback(): Promise<PlaybackControlPayload> {
  try {
    return await invoke<PlaybackControlPayload>("emergency_stop_playback");
  } catch (error) {
    if (isTauriRuntime()) throw error;
    return {
      status: "stopped",
      label: "已停止",
      reason: "emergencyStopped",
      message: "已触发紧急停止。",
    };
  }
}

export async function focusControl(): Promise<void> {
  try {
    await invoke("focus_control");
  } catch (error) {
    if (isTauriRuntime()) throw error;
    window.location.hash = "#/control";
  }
}

export async function setBackendStatus(
  status: AppStatus,
): Promise<AppStatusPayload> {
  try {
    return await invoke<AppStatusPayload>(statusCommand[status]);
  } catch (error) {
    if (isTauriRuntime()) throw error;
    const labels: Record<AppStatus, string> = {
      ready: "就绪",
      recording: "录制中",
      playing: "回放中",
      stopped: "已停止",
    };

    return { status, label: labels[status] };
  }
}
