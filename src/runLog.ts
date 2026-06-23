import type {
  PlaybackControlPayload,
  PlaybackFinishedPayload,
  PlaybackFinishReason,
  PlaybackStartPayload,
} from "./types";

export type RunLogLevel = "info" | "success" | "warning" | "danger";

export interface RunLogEntry {
  id: string;
  time: number;
  level: RunLogLevel;
  title: string;
  detail: string;
  flowName?: string;
  runId?: number;
  reason?: PlaybackFinishReason;
  completedSteps?: number;
  skippedSteps?: number;
}

const DEFAULT_MAX_RUN_LOGS = 8;

export function appendRunLog(
  entries: RunLogEntry[],
  entry: RunLogEntry,
  maxEntries = DEFAULT_MAX_RUN_LOGS,
) {
  const withoutDuplicate = entries.filter((current) => current.id !== entry.id);
  return [entry, ...withoutDuplicate].slice(0, maxEntries);
}

export function appendPlaybackStartLog(
  entries: RunLogEntry[],
  payload: PlaybackStartPayload,
  time = Date.now(),
) {
  const loopLabel =
    payload.loopCount === 0 ? "无限循环" : `${payload.loopCount} 次`;
  return appendRunLog(entries, {
    id: `playback-start-${payload.runId}`,
    time,
    level: "info",
    title: "开始回放",
    detail: `${loopLabel} · ${payload.speedMultiplier}x · ${payload.message}`,
    flowName: payload.flowName,
    runId: payload.runId,
  });
}

export function appendPlaybackControlLog(
  entries: RunLogEntry[],
  payload: PlaybackControlPayload,
  flowName: string,
  runId?: number | null,
  time = Date.now(),
) {
  return appendRunLog(entries, {
    id: `playback-control-${runId ?? "unknown"}-${payload.reason}`,
    time,
    level: playbackReasonLevel(payload.reason),
    title: playbackReasonTitle(payload.reason),
    detail: payload.message,
    flowName,
    runId: runId ?? undefined,
    reason: payload.reason,
  });
}

export function appendPlaybackFinishedLog(
  entries: RunLogEntry[],
  payload: PlaybackFinishedPayload,
  time = Date.now(),
) {
  return appendRunLog(entries, {
    id: `playback-finished-${payload.runId}`,
    time,
    level: playbackReasonLevel(payload.reason),
    title: playbackReasonTitle(payload.reason),
    detail: payload.message,
    flowName: payload.flowName,
    runId: payload.runId,
    reason: payload.reason,
    completedSteps: payload.completedSteps,
    skippedSteps: payload.skippedSteps,
  });
}

export function findLatestSafetyStopLog(
  entries: RunLogEntry[],
  flowName?: string,
) {
  for (const entry of entries) {
    if (flowName && entry.flowName !== flowName) continue;
    if (entry.reason === "safetyStopped") return entry;
    if (entry.reason || entry.title === "开始回放") return undefined;
  }

  return undefined;
}

function playbackReasonLevel(reason: PlaybackFinishReason): RunLogLevel {
  if (reason === "completed") return "success";
  if (reason === "emergencyStopped" || reason === "safetyStopped") {
    return "danger";
  }
  return "warning";
}

function playbackReasonTitle(reason: PlaybackFinishReason) {
  if (reason === "completed") return "回放完成";
  if (reason === "emergencyStopped") return "紧急停止";
  if (reason === "safetyStopped") return "安全停止";
  return "回放停止";
}
