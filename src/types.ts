export type AppStatus = "ready" | "recording" | "playing" | "stopped";
export type SaveStatus = "idle" | "saving" | "saved" | "error";

export interface AppStatusPayload {
  status: AppStatus;
  label: string;
}

export interface RecordingStartPayload extends AppStatusPayload {
  startedAt: number;
  warning: string;
  safetyWarning: string;
}

export interface RecordingStopPayload extends AppStatusPayload {
  startedAt: number;
  stoppedAt: number;
  flow: Flow;
  message: string;
}

export type PlaybackFinishReason =
  | "completed"
  | "stopped"
  | "emergencyStopped"
  | "safetyStopped";

export interface PlaybackStartPayload extends AppStatusPayload {
  runId: number;
  flowName: string;
  loopCount: number;
  speedMultiplier: number;
  message: string;
}

export interface PlaybackControlPayload extends AppStatusPayload {
  reason: PlaybackFinishReason;
  message: string;
}

export interface PlaybackFinishedPayload extends AppStatusPayload {
  runId: number;
  reason: PlaybackFinishReason;
  flowName: string;
  completedSteps: number;
  skippedSteps: number;
  loopCount: number;
  message: string;
}

export type FlowStep =
  | ClickStep
  | DragStep
  | TypeStep
  | KeyStep
  | WaitStep
  | HotkeyStep
  | ScrollStep;

export interface BaseStep {
  id: number;
  delayMs: number;
  note: string;
}

export interface ClickStep extends BaseStep {
  type: "click";
  action: "左键单击" | "右键单击" | "双击";
  target: string;
  x: number;
  y: number;
}

export interface DragStep extends BaseStep {
  type: "drag";
  action: "左键拖拽" | "右键拖拽";
  target: string;
  startX: number;
  startY: number;
  endX: number;
  endY: number;
  durationMs: number;
}

export interface TypeStep extends BaseStep {
  type: "type";
  action: "文本输入";
  text: string;
}

export interface KeyStep extends BaseStep {
  type: "key";
  action: "按键";
  key: string;
}

export interface WaitStep extends BaseStep {
  type: "wait";
  action: "等待";
  durationMs: number;
}

export interface HotkeyStep extends BaseStep {
  type: "hotkey";
  action: "快捷键";
  keys: string[];
}

export interface ScrollStep extends BaseStep {
  type: "scroll";
  action: "滚动";
  deltaX: number;
  deltaY: number;
}

export interface Flow {
  version: 1;
  name: string;
  displayName: string;
  targetWindow: FlowTargetWindow;
  steps: FlowStep[];
}

export interface FlowTargetWindow {
  title: string;
  process: string;
  size: string;
  matched: boolean;
}

export interface SavedFlow {
  fileName: string;
  savedAt: number;
  flow: Flow;
}

export interface FlowSummary {
  fileName: string;
  name: string;
  displayName: string;
  stepCount: number;
  savedAt: number;
}
