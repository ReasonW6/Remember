import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useMemo, useRef, useState } from "react";
import type {
  AppStatus,
  FlowSummary,
  PlaybackControlPayload,
  PlaybackFinishedPayload,
  PlaybackStartPayload,
  RecordingStartPayload,
  RecordingStopPayload,
  SavedFlow,
  SaveStatus,
} from "./types";
import { sampleSavedFlow } from "./data/sampleFlow";
import {
  emergencyStopPlayback,
  getInitialFlow,
  listFlows,
  loadFlow,
  openWorkbench,
  saveFlow,
  saveFlowAs,
  setBackendStatus,
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

function parseSpeedMultiplier(speed: string) {
  const multiplier = Number(speed.replace(/x$/i, ""));
  return Number.isFinite(multiplier) && multiplier > 0 ? multiplier : 1;
}

const INFINITE_LOOP_WARNING =
  "无限循环会一直重复执行，必须确认你知道停止按钮和 Ctrl + Alt + S 急停热键。";

export function App() {
  const [route, setRoute] = useState(getRoute);
  const [savedFlow, setSavedFlow] = useState<SavedFlow>(sampleSavedFlow);
  const [flowSummaries, setFlowSummaries] = useState<FlowSummary[]>([]);
  const [status, setStatus] = useState<AppStatus>("ready");
  const [loopCount, setLoopCount] = useState(3);
  const [speed, setSpeed] = useState("1x");
  const [saveStatus, setSaveStatus] = useState<SaveStatus>("idle");
  const [saveMessage, setSaveMessage] = useState("正在读取本地流程");
  const [selectedStepId, setSelectedStepId] = useState<number | null>(null);
  const [runLogs, setRunLogs] = useState<RunLogEntry[]>([]);
  const [recordingWarningVisible, setRecordingWarningVisible] = useState(false);
  const [infiniteLoopWarningVisible, setInfiniteLoopWarningVisible] =
    useState(false);
  const [infiniteLoopConfirmed, setInfiniteLoopConfirmed] = useState(false);
  const finishedPlaybackRunIds = useRef<Set<number>>(new Set());
  const activePlaybackRunId = useRef<number | null>(null);
  const activePlaybackFlowName = useRef<string>("");

  function applyRecordingStart(payload: RecordingStartPayload) {
    setStatus(payload.status);
    setSaveStatus("idle");
    setSaveMessage(payload.warning);
  }

  function applyRecordingStop(payload: RecordingStopPayload) {
    setStatus(payload.status);
    setSavedFlow(asUnsavedRecording(payload.flow));
    setSaveStatus("idle");
    setSaveMessage(payload.message);
    setRecordingWarningVisible(false);
    setInfiniteLoopWarningVisible(false);
    setInfiniteLoopConfirmed(false);
  }

  function applyPlaybackStart(payload: PlaybackStartPayload) {
    if (finishedPlaybackRunIds.current.has(payload.runId)) return;
    activePlaybackRunId.current = payload.runId;
    activePlaybackFlowName.current = payload.flowName;
    setInfiniteLoopWarningVisible(false);
    setRunLogs((current) => appendPlaybackStartLog(current, payload));
    setStatus(payload.status);
    setSaveStatus("idle");
    setSaveMessage(payload.message);
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
    setSaveStatus("idle");
    setSaveMessage(payload.message);
  }

  function applyPlaybackFinished(payload: PlaybackFinishedPayload) {
    finishedPlaybackRunIds.current.add(payload.runId);
    activePlaybackRunId.current = null;
    activePlaybackFlowName.current = "";
    setInfiniteLoopConfirmed(false);
    setRunLogs((current) => appendPlaybackFinishedLog(current, payload));
    setStatus(payload.status);
    setSaveStatus("idle");
    setSaveMessage(payload.message);
  }

  useEffect(() => {
    let cancelled = false;
    const unlisteners: UnlistenFn[] = [];

    async function hydrateFlows() {
      try {
        const initialFlow = await getInitialFlow();
        if (cancelled) return;
        setSavedFlow(initialFlow);
        setSaveStatus(initialFlow.savedAt ? "saved" : "idle");
        setSaveMessage(`本地流程: ${formatSavedAt(initialFlow.savedAt)}`);

        const summaries = await listFlows();
        if (!cancelled) setFlowSummaries(summaries);
      } catch (error) {
        if (cancelled) return;
        setSaveStatus("error");
        setSaveMessage(String(error));
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

  async function updateStatus(nextStatus: AppStatus) {
    if (nextStatus === "recording") {
      setRecordingWarningVisible(true);
      setSaveStatus("idle");
      setSaveMessage(RECORDING_SAFETY_WARNING);
      return;
    }

    if (nextStatus === "stopped" && status === "recording") {
      try {
        const payload = await stopRecording();
        applyRecordingStop(payload);
        await openWorkbench();
      } catch (error) {
        setSaveStatus("error");
        setSaveMessage(String(error));
      }
      return;
    }

    if (nextStatus === "playing") {
      if (loopCount === 0 && !infiniteLoopConfirmed) {
        setInfiniteLoopWarningVisible(true);
        setSaveStatus("idle");
        setSaveMessage(INFINITE_LOOP_WARNING);
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
        setSaveStatus("error");
        setSaveMessage(String(error));
      }
      return;
    }

    if (nextStatus === "stopped" && status === "playing") {
      try {
        const payload = await stopPlayback();
        applyPlaybackStop(payload);
      } catch (error) {
        setSaveStatus("error");
        setSaveMessage(String(error));
      }
      return;
    }

    const payload = await setBackendStatus(nextStatus);
    setStatus(payload.status);
  }

  async function confirmRecordingStart() {
    setRecordingWarningVisible(false);
    try {
      const payload = await startRecording();
      applyRecordingStart(payload);
    } catch (error) {
      setSaveStatus("error");
      setSaveMessage(String(error));
    }
  }

  function cancelRecordingStart() {
    setRecordingWarningVisible(false);
    setSaveStatus("idle");
    setSaveMessage("已取消录制");
  }

  function handleLoopCountChange(value: number) {
    const nextLoopCount = Number.isFinite(value)
      ? Math.max(0, Math.floor(value))
      : 1;

    if (nextLoopCount === 0) {
      setLoopCount(0);
      setInfiniteLoopConfirmed(false);
      setInfiniteLoopWarningVisible(true);
      setSaveStatus("idle");
      setSaveMessage(INFINITE_LOOP_WARNING);
      return;
    }

    setLoopCount(nextLoopCount);
    setInfiniteLoopConfirmed(false);
    setInfiniteLoopWarningVisible(false);
  }

  function confirmInfiniteLoop() {
    setLoopCount(0);
    setInfiniteLoopConfirmed(true);
    setInfiniteLoopWarningVisible(false);
    setSaveStatus("idle");
    setSaveMessage("已确认无限循环；运行后请使用停止或 Ctrl + Alt + S 结束。");
  }

  function cancelInfiniteLoop() {
    setLoopCount(1);
    setInfiniteLoopConfirmed(false);
    setInfiniteLoopWarningVisible(false);
    setSaveStatus("idle");
    setSaveMessage("已取消无限循环");
  }

  async function refreshFlowSummaries() {
    const summaries = await listFlows();
    setFlowSummaries(summaries);
  }

  async function handleSaveFlow() {
    setSaveStatus("saving");
    setSaveMessage("正在保存到本地");
    try {
      const nextSavedFlow = await saveFlow(flow);
      setSavedFlow(nextSavedFlow);
      setSaveStatus("saved");
      setSaveMessage(`已保存: ${formatSavedAt(nextSavedFlow.savedAt)}`);
      await refreshFlowSummaries();
    } catch (error) {
      setSaveStatus("error");
      setSaveMessage(String(error));
    }
  }

  async function handleSaveFlowAs() {
    setSaveStatus("saving");
    setSaveMessage("正在另存为本地副本");
    try {
      const nextSavedFlow = await saveFlowAs(flow, `${flow.displayName} Copy`);
      setSavedFlow(nextSavedFlow);
      setSaveStatus("saved");
      setSaveMessage(`已另存为: ${formatSavedAt(nextSavedFlow.savedAt)}`);
      await refreshFlowSummaries();
    } catch (error) {
      setSaveStatus("error");
      setSaveMessage(String(error));
    }
  }

  function handleFlowDisplayNameChange(displayName: string) {
    setSavedFlow((current) => ({
      ...current,
      flow: {
        ...current.flow,
        displayName,
      },
    }));
    setSaveStatus("idle");
    setSaveMessage("有未保存更改");
  }

  function handleStepDelayChange(stepId: number, delayMs: number) {
    setSavedFlow((current) => ({
      ...current,
      flow: updateStepDelayMs(current.flow, stepId, delayMs),
    }));
    setSelectedStepId(stepId);
    setSaveStatus("idle");
    setSaveMessage("有未保存更改");
  }

  function handleStepClickCoordinatesChange(stepId: number, x: number, y: number) {
    setSavedFlow((current) => ({
      ...current,
      flow: updateStepClickCoordinates(current.flow, stepId, x, y),
    }));
    setSelectedStepId(stepId);
    setSaveStatus("idle");
    setSaveMessage("有未保存更改");
  }

  function handleStepTextChange(stepId: number, text: string) {
    setSavedFlow((current) => ({
      ...current,
      flow: updateStepText(current.flow, stepId, text),
    }));
    setSelectedStepId(stepId);
    setSaveStatus("idle");
    setSaveMessage("有未保存更改");
  }

  function handleStepHotkeyChange(stepId: number, hotkeyText: string) {
    setSavedFlow((current) => ({
      ...current,
      flow: updateStepHotkeyText(current.flow, stepId, hotkeyText),
    }));
    setSelectedStepId(stepId);
    setSaveStatus("idle");
    setSaveMessage("有未保存更改");
  }

  function handleStepKeyChange(stepId: number, keyText: string) {
    setSavedFlow((current) => ({
      ...current,
      flow: updateStepKeyText(current.flow, stepId, keyText),
    }));
    setSelectedStepId(stepId);
    setSaveStatus("idle");
    setSaveMessage("有未保存更改");
  }

  function handleTargetWindowMatchedChange(matched: boolean) {
    setSavedFlow((current) => ({
      ...current,
      flow: updateTargetWindowMatched(current.flow, matched),
    }));
    setSaveStatus("idle");
    setSaveMessage("有未保存更改");
  }

  function handleStepDelete(stepId: number) {
    setSavedFlow((current) => {
      const result = deleteStep(current.flow, stepId);
      setSelectedStepId(result.selectedStepId);
      return {
        ...current,
        flow: result.flow,
      };
    });
    setSaveStatus("idle");
    setSaveMessage("有未保存更改");
  }

  function handleInsertWaitStep() {
    setSavedFlow((current) => {
      const result = insertWaitStepAfter(current.flow, selectedStepId);
      setSelectedStepId(result.selectedStepId);
      return {
        ...current,
        flow: result.flow,
      };
    });
    setSaveStatus("idle");
    setSaveMessage("有未保存更改");
  }

  function handleInsertTypeStep() {
    setSavedFlow((current) => {
      const result = insertTypeStepAfter(current.flow, selectedStepId);
      setSelectedStepId(result.selectedStepId);
      return {
        ...current,
        flow: result.flow,
      };
    });
    setSaveStatus("idle");
    setSaveMessage("有未保存更改");
  }

  function handleInsertHotkeyStep() {
    setSavedFlow((current) => {
      const result = insertHotkeyStepAfter(current.flow, selectedStepId);
      setSelectedStepId(result.selectedStepId);
      return {
        ...current,
        flow: result.flow,
      };
    });
    setSaveStatus("idle");
    setSaveMessage("有未保存更改");
  }

  function handleInsertKeyStep() {
    setSavedFlow((current) => {
      const result = insertKeyStepAfter(current.flow, selectedStepId);
      setSelectedStepId(result.selectedStepId);
      return {
        ...current,
        flow: result.flow,
      };
    });
    setSaveStatus("idle");
    setSaveMessage("有未保存更改");
  }

  async function handleLoadFlow(fileName: string) {
    setSaveStatus("idle");
    setSaveMessage("正在加载本地流程");
    try {
      const nextSavedFlow = await loadFlow(fileName);
      setSavedFlow(nextSavedFlow);
      setInfiniteLoopWarningVisible(false);
      setInfiniteLoopConfirmed(false);
      setSaveStatus(nextSavedFlow.savedAt ? "saved" : "idle");
      setSaveMessage(`已加载: ${formatSavedAt(nextSavedFlow.savedAt)}`);
    } catch (error) {
      setSaveStatus("error");
      setSaveMessage(String(error));
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
      setSaveStatus("error");
      setSaveMessage(String(error));
    }
  }

  const sharedProps = {
    flow,
    flowSummaries,
    selectedFileName: savedFlow.fileName,
    savedAt: savedFlow.savedAt,
    saveStatus,
    saveMessage,
    selectedStepId,
    runLogs,
    status,
    statusLabel,
    loopCount,
    speed,
    onLoopCountChange: handleLoopCountChange,
    onSpeedChange: setSpeed,
    onStatusChange: updateStatus,
    onFlowSelect: handleLoadFlow,
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
    onOpenWorkbench: openWorkbench,
    onEmergencyStop: handleEmergencyStop,
    recordingWarningVisible,
    recordingSafetyWarning: RECORDING_SAFETY_WARNING,
    onConfirmRecordingStart: confirmRecordingStart,
    onCancelRecordingStart: cancelRecordingStart,
    infiniteLoopWarningVisible,
    infiniteLoopConfirmed,
    infiniteLoopWarning: INFINITE_LOOP_WARNING,
    onConfirmInfiniteLoop: confirmInfiniteLoop,
    onCancelInfiniteLoop: cancelInfiniteLoop,
  };

  return route === "workbench" ? (
    <WorkbenchWindow {...sharedProps} />
  ) : (
    <ControlWindow {...sharedProps} />
  );
}
