import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useMemo, useRef, useState } from "react";
import type {
  AppStatus,
  EmergencyHotkeyStatusPayload,
  FlowSummary,
  PlaybackControlPayload,
  PlaybackFinishedPayload,
  PlaybackStartPayload,
  RecordingStartPayload,
  RecordingStopPayload,
  SavedFlow,
  SaveStatus,
} from "./types";
import {
  emergencyStopPlayback,
  EMERGENCY_HOTKEY_SHORTCUT,
  getEmergencyHotkeyStatus,
  getInitialFlow,
  listFlows,
  loadFlow,
  openWorkbench,
  saveFlow,
  saveFlowAs,
  startPlayback,
  startRecording,
  stopPlayback,
  stopRecording,
  RECORDING_SAFETY_WARNING,
} from "./tauriApi";
import { ControlWindow } from "./components/ControlWindow";
import { WorkbenchWindow } from "./components/WorkbenchWindow";
import {
  deleteStep,
  insertHotkeyStepAfter,
  insertKeyStepAfter,
  insertTypeStepAfter,
  insertWaitStepAfter,
  selectExistingStepId,
  updateStepClickCoordinates,
  updateStepDelayMs,
  updateStepHotkeyText,
  updateStepKeyText,
  updateStepText,
  updateTargetWindowMatched,
} from "./flowEditing";
import { parseSpeedMultiplier } from "./flowTiming";
import {
  appendPlaybackControlLog,
  appendPlaybackFinishedLog,
  appendPlaybackStartLog,
  type RunLogEntry,
} from "./runLog";

function getRoute() {
  const route = window.location.hash.replace(/^#\/?/, "");
  return route === "workbench" ? "workbench" : "control";
}

function formatSavedAt(savedAt: number) {
  if (!savedAt) return "尚未保存";
  return new Date(savedAt * 1000).toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function asUnsavedRecording(flow: RecordingStopPayload["flow"]): SavedFlow {
  return {
    fileName: `${flow.name}.remember.json`,
    savedAt: 0,
    flow,
  };
}

const INFINITE_LOOP_WARNING =
  "无限循环会一直重复执行，必须确认你知道停止按钮和 Ctrl + Alt + S 急停热键。";
const FLOW_DRAFT_UPDATED_EVENT = "flow-draft-updated";
const PLAYBACK_OPTIONS_UPDATED_EVENT = "playback-options-updated";

interface PlaybackOptionsDraft {
  loopCount: number;
  speed: string;
  infiniteLoopConfirmed: boolean;
}

const initialSavedFlow: SavedFlow = {
  fileName: "loading.remember.json",
  savedAt: 0,
  flow: {
    version: 1,
    name: "loading",
    displayName: "正在加载本地流程",
    targetWindow: {
      title: "尚未加载",
      process: "N/A",
      size: "N/A",
      matched: false,
    },
    steps: [],
  },
};

export function App() {
  const [route, setRoute] = useState(getRoute);
  const [savedFlow, setSavedFlow] = useState<SavedFlow>(initialSavedFlow);
  const [flowSummaries, setFlowSummaries] = useState<FlowSummary[]>([]);
  const [status, setStatus] = useState<AppStatus>("ready");
  const [loopCount, setLoopCount] = useState(3);
  const [speed, setSpeed] = useState("1x");
  const [isFlowLoading, setIsFlowLoading] = useState(false);
  const [saveStatus, setSaveStatus] = useState<SaveStatus>("idle");
  const [saveMessage, setSaveMessage] = useState("正在读取本地流程");
  const [selectedStepId, setSelectedStepId] = useState<number | null>(null);
  const [runLogs, setRunLogs] = useState<RunLogEntry[]>([]);
  const [targetSafetyRunId, setTargetSafetyRunId] = useState<number | null>(null);
  const [recordingWarningVisible, setRecordingWarningVisible] = useState(false);
  const [infiniteLoopWarningVisible, setInfiniteLoopWarningVisible] =
    useState(false);
  const [infiniteLoopConfirmed, setInfiniteLoopConfirmed] = useState(false);
  const [emergencyHotkeyStatus, setEmergencyHotkeyStatus] =
    useState<EmergencyHotkeyStatusPayload>({
      available: true,
      shortcut: EMERGENCY_HOTKEY_SHORTCUT,
      message: `紧急停止热键可用: ${EMERGENCY_HOTKEY_SHORTCUT}`,
    });
  const finishedPlaybackRunIds = useRef<Set<number>>(new Set());
  const activePlaybackRunId = useRef<number | null>(null);
  const activePlaybackFlowName = useRef<string>("");
  const savedFlowRef = useRef<SavedFlow>(initialSavedFlow);

  function setCurrentSavedFlow(nextSavedFlow: SavedFlow) {
    savedFlowRef.current = nextSavedFlow;
    setSavedFlow(nextSavedFlow);
  }

  function setSaveFeedback(nextStatus: SaveStatus, message: string) {
    setSaveStatus(nextStatus);
    setSaveMessage(message);
  }

  function setSaveError(error: unknown) {
    setSaveFeedback("error", String(error));
  }

  function markUnsaved() {
    setSaveFeedback("idle", "有未保存更改");
  }

  function broadcastFlowDraft(nextSavedFlow: SavedFlow) {
    void emit<SavedFlow>(FLOW_DRAFT_UPDATED_EVENT, nextSavedFlow).catch(() => {
      // Browser preview cannot emit Tauri events.
    });
  }

  function broadcastPlaybackOptions(nextOptions: PlaybackOptionsDraft) {
    void emit<PlaybackOptionsDraft>(
      PLAYBACK_OPTIONS_UPDATED_EVENT,
      nextOptions,
    ).catch(() => {
      // Browser preview cannot emit Tauri events.
    });
  }

  function commitDraft(nextSavedFlow: SavedFlow) {
    setCurrentSavedFlow(nextSavedFlow);
    markUnsaved();
    broadcastFlowDraft(nextSavedFlow);
  }

  function applySavedFlow(
    nextSavedFlow: SavedFlow,
    messagePrefix: "已保存" | "已加载" = "已保存",
  ) {
    setCurrentSavedFlow(nextSavedFlow);
    setSaveFeedback(
      nextSavedFlow.savedAt ? "saved" : "idle",
      `${messagePrefix}: ${formatSavedAt(nextSavedFlow.savedAt)}`,
    );
  }

  function applyRecordingStart(payload: RecordingStartPayload) {
    setStatus(payload.status);
    setSaveFeedback("idle", payload.warning);
  }

  function applyRecordingStop(payload: RecordingStopPayload) {
    setStatus(payload.status);
    setCurrentSavedFlow(asUnsavedRecording(payload.flow));
    setSaveFeedback("idle", payload.message);
    setRecordingWarningVisible(false);
    setInfiniteLoopWarningVisible(false);
    setInfiniteLoopConfirmed(false);
  }

  function applyPlaybackStart(payload: PlaybackStartPayload) {
    if (finishedPlaybackRunIds.current.has(payload.runId)) return;
    activePlaybackRunId.current = payload.runId;
    activePlaybackFlowName.current = payload.flowName;
    setTargetSafetyRunId(null);
    setInfiniteLoopWarningVisible(false);
    setRunLogs((current) => appendPlaybackStartLog(current, payload));
    setStatus(payload.status);
    setSaveFeedback("idle", payload.message);
  }

  function applyPlaybackStop(payload: PlaybackControlPayload) {
    setRunLogs((current) =>
      appendPlaybackControlLog(
        current,
        payload,
        activePlaybackFlowName.current || "当前流程",
        activePlaybackRunId.current,
      ),
    );
    setInfiniteLoopConfirmed(false);
    setStatus(payload.status);
    setSaveFeedback("idle", payload.message);
  }

  function applyPlaybackFinished(payload: PlaybackFinishedPayload) {
    finishedPlaybackRunIds.current.add(payload.runId);
    activePlaybackRunId.current = null;
    activePlaybackFlowName.current = "";
    setTargetSafetyRunId(
      payload.reason === "safetyStopped" ? payload.runId : null,
    );
    setInfiniteLoopConfirmed(false);
    setRunLogs((current) => appendPlaybackFinishedLog(current, payload));
    setStatus(payload.status);
    setSaveFeedback("idle", payload.message);
  }

  useEffect(() => {
    let cancelled = false;
    const unlisteners: UnlistenFn[] = [];

    async function hydrateFlows() {
      try {
        const initialFlow = await getInitialFlow();
        if (cancelled) return;
        setCurrentSavedFlow(initialFlow);
        setSaveFeedback(
          initialFlow.savedAt ? "saved" : "idle",
          `本地流程: ${formatSavedAt(initialFlow.savedAt)}`,
        );

        const hotkeyStatus = await getEmergencyHotkeyStatus();
        if (!cancelled) setEmergencyHotkeyStatus(hotkeyStatus);

        const summaries = await listFlows();
        if (!cancelled) setFlowSummaries(summaries);
      } catch (error) {
        if (cancelled) return;
        setSaveError(error);
      }
    }

    async function attachRecordingEvents() {
      try {
        const unlistenStarted = await listen<RecordingStartPayload>(
          "recording-started",
          (event) => {
            if (!cancelled) applyRecordingStart(event.payload);
          },
        );
        if (cancelled) {
          unlistenStarted();
          return;
        }
        unlisteners.push(unlistenStarted);

        const unlistenStopped = await listen<RecordingStopPayload>(
          "recording-stopped",
          (event) => {
            if (!cancelled) applyRecordingStop(event.payload);
          },
        );
        if (cancelled) {
          unlistenStopped();
          return;
        }
        unlisteners.push(unlistenStopped);

        const unlistenPlaybackStarted = await listen<PlaybackStartPayload>(
          "playback-started",
          (event) => {
            if (!cancelled) applyPlaybackStart(event.payload);
          },
        );
        if (cancelled) {
          unlistenPlaybackStarted();
          return;
        }
        unlisteners.push(unlistenPlaybackStarted);

        const unlistenPlaybackStopped = await listen<PlaybackControlPayload>(
          "playback-stopped",
          (event) => {
            if (!cancelled) applyPlaybackStop(event.payload);
          },
        );
        if (cancelled) {
          unlistenPlaybackStopped();
          return;
        }
        unlisteners.push(unlistenPlaybackStopped);

        const unlistenPlaybackFinished = await listen<PlaybackFinishedPayload>(
          "playback-finished",
          (event) => {
            if (!cancelled) applyPlaybackFinished(event.payload);
          },
        );
        if (cancelled) {
          unlistenPlaybackFinished();
          return;
        }
        unlisteners.push(unlistenPlaybackFinished);

        const unlistenFlowSaved = await listen<SavedFlow>(
          "flow-saved",
          (event) => {
            if (cancelled) return;
            applySavedFlow(event.payload, "已保存");
            void refreshFlowSummaries().catch((error) => {
              setSaveError(error);
            });
          },
        );
        if (cancelled) {
          unlistenFlowSaved();
          return;
        }
        unlisteners.push(unlistenFlowSaved);

        const unlistenFlowLoaded = await listen<SavedFlow>(
          "flow-loaded",
          (event) => {
            if (cancelled) return;
            applySavedFlow(event.payload, "已加载");
          },
        );
        if (cancelled) {
          unlistenFlowLoaded();
          return;
        }
        unlisteners.push(unlistenFlowLoaded);

        const unlistenDraftUpdated = await listen<SavedFlow>(
          FLOW_DRAFT_UPDATED_EVENT,
          (event) => {
            if (cancelled) return;
            setCurrentSavedFlow(event.payload);
            setSaveFeedback("idle", "有未保存更改");
          },
        );
        if (cancelled) {
          unlistenDraftUpdated();
          return;
        }
        unlisteners.push(unlistenDraftUpdated);

        const unlistenPlaybackOptionsUpdated =
          await listen<PlaybackOptionsDraft>(
            PLAYBACK_OPTIONS_UPDATED_EVENT,
            (event) => {
              if (cancelled) return;
              setLoopCount(event.payload.loopCount);
              setSpeed(event.payload.speed);
              setInfiniteLoopConfirmed(event.payload.infiniteLoopConfirmed);
            },
          );
        if (cancelled) {
          unlistenPlaybackOptionsUpdated();
          return;
        }
        unlisteners.push(unlistenPlaybackOptionsUpdated);
      } catch {
        // The browser preview fallback cannot register Tauri events.
      }
    }

    hydrateFlows();
    attachRecordingEvents();

    const onHashChange = () => setRoute(getRoute());
    window.addEventListener("hashchange", onHashChange);
    return () => {
      cancelled = true;
      unlisteners.forEach((unlisten) => unlisten());
      window.removeEventListener("hashchange", onHashChange);
    };
  }, []);

  const flow = savedFlow.flow;

  useEffect(() => {
    setSelectedStepId((current) => selectExistingStepId(flow, current));
  }, [flow]);

  const statusLabel = useMemo(() => {
    const labels: Record<AppStatus, string> = {
      ready: "就绪",
      recording: "录制中",
      playing: "回放中",
      stopped: "已停止",
    };

    return labels[status];
  }, [status]);

  const emergencyStopHint = emergencyHotkeyStatus.available
    ? `紧急停止: ${emergencyHotkeyStatus.shortcut}`
    : "紧急停止热键不可用，请使用停止按钮";

  const infiniteLoopWarning = emergencyHotkeyStatus.available
    ? INFINITE_LOOP_WARNING
    : "无限循环会一直重复执行；当前全局急停热键不可用，必须确认你能使用窗口里的停止按钮。";

  async function updateStatus(nextStatus: AppStatus) {
    if (isFlowLoading && nextStatus === "playing") {
      setSaveFeedback("idle", "正在加载本地流程，加载完成后再运行。");
      return;
    }

    if (nextStatus === "recording") {
      setRecordingWarningVisible(true);
      setSaveFeedback("idle", RECORDING_SAFETY_WARNING);
      return;
    }

    if (nextStatus === "stopped" && status === "recording") {
      try {
        const payload = await stopRecording();
        applyRecordingStop(payload);
        await openWorkbench();
      } catch (error) {
        setSaveError(error);
      }
      return;
    }

    if (nextStatus === "playing") {
      if (loopCount === 0 && !infiniteLoopConfirmed) {
        setInfiniteLoopWarningVisible(true);
        setSaveFeedback("idle", infiniteLoopWarning);
        return;
      }

      try {
        const payload = await startPlayback(
          flow,
          parseSpeedMultiplier(speed),
          loopCount,
          loopCount === 0 && infiniteLoopConfirmed,
        );
        applyPlaybackStart(payload);
      } catch (error) {
        setSaveError(error);
      }
      return;
    }

    if (nextStatus === "stopped" && status === "playing") {
      try {
        const payload = await stopPlayback();
        applyPlaybackStop(payload);
      } catch (error) {
        setSaveError(error);
      }
      return;
    }

    setStatus(nextStatus);
  }

  async function confirmRecordingStart() {
    setRecordingWarningVisible(false);
    try {
      const payload = await startRecording();
      applyRecordingStart(payload);
    } catch (error) {
      setSaveError(error);
    }
  }

  function cancelRecordingStart() {
    setRecordingWarningVisible(false);
    setSaveFeedback("idle", "已取消录制");
  }

  function handleLoopCountChange(value: number) {
    const nextLoopCount = Number.isFinite(value)
      ? Math.max(0, Math.floor(value))
      : 1;

    if (nextLoopCount === 0) {
      setLoopCount(0);
      setInfiniteLoopConfirmed(false);
      setInfiniteLoopWarningVisible(true);
      setSaveFeedback("idle", infiniteLoopWarning);
      broadcastPlaybackOptions({
        loopCount: 0,
        speed,
        infiniteLoopConfirmed: false,
      });
      return;
    }

    setLoopCount(nextLoopCount);
    setInfiniteLoopConfirmed(false);
    setInfiniteLoopWarningVisible(false);
    broadcastPlaybackOptions({
      loopCount: nextLoopCount,
      speed,
      infiniteLoopConfirmed: false,
    });
  }

  function handleSpeedChange(nextSpeed: string) {
    setSpeed(nextSpeed);
    broadcastPlaybackOptions({
      loopCount,
      speed: nextSpeed,
      infiniteLoopConfirmed,
    });
  }

  function confirmInfiniteLoop() {
    setLoopCount(0);
    setInfiniteLoopConfirmed(true);
    setInfiniteLoopWarningVisible(false);
    broadcastPlaybackOptions({
      loopCount: 0,
      speed,
      infiniteLoopConfirmed: true,
    });
    setSaveFeedback(
      "idle",
      emergencyHotkeyStatus.available
        ? `已确认无限循环；运行后请使用停止或 ${emergencyHotkeyStatus.shortcut} 结束。`
        : "已确认无限循环；运行后请使用窗口里的停止按钮结束。",
    );
  }

  function cancelInfiniteLoop() {
    setLoopCount(1);
    setInfiniteLoopConfirmed(false);
    setInfiniteLoopWarningVisible(false);
    broadcastPlaybackOptions({
      loopCount: 1,
      speed,
      infiniteLoopConfirmed: false,
    });
    setSaveFeedback("idle", "已取消无限循环");
  }

  async function refreshFlowSummaries() {
    const summaries = await listFlows();
    setFlowSummaries(summaries);
  }

  async function handleSaveFlow() {
    if (isFlowLoading) {
      setSaveFeedback("idle", "正在加载本地流程，加载完成后再保存。");
      return;
    }
    setSaveFeedback("saving", "正在保存到本地");
    try {
      const nextSavedFlow = await saveFlow(savedFlow.fileName, flow);
      applySavedFlow(nextSavedFlow, "已保存");
      await refreshFlowSummaries();
    } catch (error) {
      setSaveError(error);
    }
  }

  async function handleSaveFlowAs() {
    if (isFlowLoading) {
      setSaveFeedback("idle", "正在加载本地流程，加载完成后再保存。");
      return;
    }
    setSaveFeedback("saving", "正在另存为本地副本");
    try {
      const nextSavedFlow = await saveFlowAs(flow, `${flow.displayName} Copy`);
      applySavedFlow(nextSavedFlow, "已保存");
      await refreshFlowSummaries();
    } catch (error) {
      setSaveError(error);
    }
  }

  function handleFlowDisplayNameChange(displayName: string) {
    const current = savedFlowRef.current;
    commitDraft({
      ...current,
      flow: {
        ...current.flow,
        displayName,
      },
    });
  }

  function handleStepDelayChange(stepId: number, delayMs: number) {
    const current = savedFlowRef.current;
    commitDraft({
      ...current,
      flow: updateStepDelayMs(current.flow, stepId, delayMs),
    });
    setSelectedStepId(stepId);
  }

  function handleStepClickCoordinatesChange(stepId: number, x: number, y: number) {
    const current = savedFlowRef.current;
    commitDraft({
      ...current,
      flow: updateStepClickCoordinates(current.flow, stepId, x, y),
    });
    setSelectedStepId(stepId);
  }

  function handleStepTextChange(stepId: number, text: string) {
    const current = savedFlowRef.current;
    commitDraft({
      ...current,
      flow: updateStepText(current.flow, stepId, text),
    });
    setSelectedStepId(stepId);
  }

  function handleStepHotkeyChange(stepId: number, hotkeyText: string) {
    const current = savedFlowRef.current;
    commitDraft({
      ...current,
      flow: updateStepHotkeyText(current.flow, stepId, hotkeyText),
    });
    setSelectedStepId(stepId);
  }

  function handleStepKeyChange(stepId: number, keyText: string) {
    const current = savedFlowRef.current;
    commitDraft({
      ...current,
      flow: updateStepKeyText(current.flow, stepId, keyText),
    });
    setSelectedStepId(stepId);
  }

  function handleTargetWindowMatchedChange(matched: boolean) {
    const current = savedFlowRef.current;
    commitDraft({
      ...current,
      flow: updateTargetWindowMatched(current.flow, matched),
    });
    setTargetSafetyRunId(null);
  }

  function handleStepDelete(stepId: number) {
    const current = savedFlowRef.current;
    const result = deleteStep(current.flow, stepId);
    commitDraft({
      ...current,
      flow: result.flow,
    });
    setSelectedStepId(result.selectedStepId);
  }

  function handleInsertWaitStep() {
    const current = savedFlowRef.current;
    const result = insertWaitStepAfter(current.flow, selectedStepId);
    commitDraft({
      ...current,
      flow: result.flow,
    });
    setSelectedStepId(result.selectedStepId);
  }

  function handleInsertTypeStep() {
    const current = savedFlowRef.current;
    const result = insertTypeStepAfter(current.flow, selectedStepId);
    commitDraft({
      ...current,
      flow: result.flow,
    });
    setSelectedStepId(result.selectedStepId);
  }

  function handleInsertHotkeyStep() {
    const current = savedFlowRef.current;
    const result = insertHotkeyStepAfter(current.flow, selectedStepId);
    commitDraft({
      ...current,
      flow: result.flow,
    });
    setSelectedStepId(result.selectedStepId);
  }

  function handleInsertKeyStep() {
    const current = savedFlowRef.current;
    const result = insertKeyStepAfter(current.flow, selectedStepId);
    commitDraft({
      ...current,
      flow: result.flow,
    });
    setSelectedStepId(result.selectedStepId);
  }

  async function handleLoadFlow(fileName: string) {
    if (isFlowLoading) return;
    setIsFlowLoading(true);
    setSaveFeedback("idle", "正在加载本地流程");
    try {
      const nextSavedFlow = await loadFlow(fileName);
      setInfiniteLoopWarningVisible(false);
      setInfiniteLoopConfirmed(false);
      setTargetSafetyRunId(null);
      applySavedFlow(nextSavedFlow, "已加载");
    } catch (error) {
      setSaveError(error);
    } finally {
      setIsFlowLoading(false);
    }
  }

  async function handleEmergencyStop() {
    if (status !== "playing") {
      await updateStatus("stopped");
      return;
    }

    try {
      const payload = await emergencyStopPlayback();
      applyPlaybackStop(payload);
    } catch (error) {
      setSaveError(error);
    }
  }

  const controlProps = {
    flow,
    flowSummaries,
    selectedFileName: savedFlow.fileName,
    status,
    statusLabel,
    loopCount,
    speed,
    isFlowLoading,
    onLoopCountChange: handleLoopCountChange,
    onSpeedChange: handleSpeedChange,
    onStatusChange: updateStatus,
    onFlowSelect: handleLoadFlow,
    onOpenWorkbench: openWorkbench,
    emergencyStopHint,
    recordingWarningVisible,
    recordingSafetyWarning: RECORDING_SAFETY_WARNING,
    onConfirmRecordingStart: confirmRecordingStart,
    onCancelRecordingStart: cancelRecordingStart,
    infiniteLoopWarningVisible,
    infiniteLoopConfirmed,
    infiniteLoopWarning,
    onConfirmInfiniteLoop: confirmInfiniteLoop,
    onCancelInfiniteLoop: cancelInfiniteLoop,
  };

  const workbenchProps = {
    flow,
    selectedFileName: savedFlow.fileName,
    savedAt: savedFlow.savedAt,
    saveStatus,
    saveMessage,
    isFlowLoading,
    targetSafetyRunId,
    selectedStepId,
    runLogs,
    status,
    statusLabel,
    loopCount,
    speed,
    onLoopCountChange: handleLoopCountChange,
    onSpeedChange: handleSpeedChange,
    onStatusChange: updateStatus,
    onFlowDisplayNameChange: handleFlowDisplayNameChange,
    onStepSelect: setSelectedStepId,
    onStepDelayChange: handleStepDelayChange,
    onStepClickCoordinatesChange: handleStepClickCoordinatesChange,
    onStepTextChange: handleStepTextChange,
    onStepHotkeyChange: handleStepHotkeyChange,
    onStepKeyChange: handleStepKeyChange,
    onTargetWindowMatchedChange: handleTargetWindowMatchedChange,
    onStepDelete: handleStepDelete,
    onInsertWaitStep: handleInsertWaitStep,
    onInsertTypeStep: handleInsertTypeStep,
    onInsertHotkeyStep: handleInsertHotkeyStep,
    onInsertKeyStep: handleInsertKeyStep,
    onSaveFlow: handleSaveFlow,
    onSaveFlowAs: handleSaveFlowAs,
    onEmergencyStop: handleEmergencyStop,
    emergencyStopHint,
    infiniteLoopWarningVisible,
    infiniteLoopConfirmed,
    infiniteLoopWarning,
    onConfirmInfiniteLoop: confirmInfiniteLoop,
    onCancelInfiniteLoop: cancelInfiniteLoop,
  };

  return route === "workbench" ? (
    <WorkbenchWindow {...workbenchProps} />
  ) : (
    <ControlWindow {...controlProps} />
  );
}
