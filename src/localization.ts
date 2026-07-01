import type { AppMode } from "./types";

const modeLabels: Record<AppMode, string> = {
  idle: "就绪",
  recording: "正在录制",
  playing: "正在回放"
};

const messageLabels: Record<string, string> = {
  Idle: "就绪",
  Recording: "正在录制",
  "Recording stopped": "录制已停止",
  "Recording loaded": "录制已载入",
  Playing: "正在回放",
  "Playback stopped": "回放已停止",
  "Playback finished": "回放完成",
  Ready: "就绪",
  "Loaded recording": "录制已载入"
};

export function displayMode(mode: AppMode) {
  return modeLabels[mode];
}

export function displayMessage(message: string) {
  return messageLabels[message] ?? message;
}
