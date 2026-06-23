import {
  Circle,
  Play,
  Settings,
  Square,
  Keyboard,
} from "lucide-react";
import type { AppStatus, Flow, FlowSummary } from "../types";

interface ControlWindowProps {
  flow: Flow;
  flowSummaries: FlowSummary[];
  selectedFileName: string;
  status: AppStatus;
  statusLabel: string;
  loopCount: number;
  speed: string;
  onLoopCountChange: (value: number) => void;
  onSpeedChange: (value: string) => void;
  onStatusChange: (status: AppStatus) => void;
  onFlowSelect: (fileName: string) => void;
  onOpenWorkbench: () => void;
  recordingWarningVisible: boolean;
  recordingSafetyWarning: string;
  onConfirmRecordingStart: () => void;
  onCancelRecordingStart: () => void;
}

export function ControlWindow({
  flow,
  flowSummaries,
  selectedFileName,
  status,
  statusLabel,
  loopCount,
  speed,
  onLoopCountChange,
  onSpeedChange,
  onStatusChange,
  onFlowSelect,
  onOpenWorkbench,
  recordingWarningVisible,
  recordingSafetyWarning,
  onConfirmRecordingStart,
  onCancelRecordingStart,
}: ControlWindowProps) {
  const currentFlowSummary = {
    fileName: selectedFileName,
    name: flow.name,
    displayName: flow.displayName,
    stepCount: flow.steps.length,
    savedAt: 0,
  };
  const flowOptions = flowSummaries.some(
    (summary) => summary.fileName === selectedFileName,
  )
    ? flowSummaries
    : [currentFlowSummary, ...flowSummaries];

  return (
    <main className="control-window">
      <section className="control-card">
        <div className="mini-body">
          <label className="field flow-field">
            <span>当前流程</span>
            <select
              aria-label="当前流程"
              value={selectedFileName}
              onChange={(event) => onFlowSelect(event.target.value)}
            >
              {flowOptions.map((summary) => (
                <option value={summary.fileName} key={summary.fileName}>
                  {summary.displayName}
                </option>
              ))}
            </select>
          </label>

          <button
            className="action-button record"
            disabled={status === "recording"}
            onClick={() => onStatusChange("recording")}
          >
            <Circle size={17} fill="currentColor" />
            <span>录制</span>
          </button>
          <button
            className="action-button replay"
            disabled={status === "recording" || status === "playing"}
            onClick={() => onStatusChange("playing")}
          >
            <Play size={18} fill="currentColor" />
            <span>重放</span>
          </button>
          <button
            className="action-button stop"
            disabled={status === "ready" || status === "stopped"}
            onClick={() => onStatusChange("stopped")}
          >
            <Square size={15} fill="currentColor" />
            <span>停止</span>
          </button>

          <label className="field tiny-field">
            <span>速度</span>
            <select
              aria-label="速度"
              value={speed}
              onChange={(event) => onSpeedChange(event.target.value)}
            >
              <option>0.5x</option>
              <option>1x</option>
              <option>2x</option>
              <option>5x</option>
            </select>
          </label>

          <label className="field tiny-field">
            <span>次数</span>
            <select
              aria-label="次数"
              value={loopCount}
              onChange={(event) => onLoopCountChange(Number(event.target.value))}
            >
              <option value={1}>1</option>
              <option value={3}>3</option>
              <option value={10}>10</option>
            </select>
          </label>

          <button
            className="icon-button settings-button"
            aria-label="设置"
            onClick={onOpenWorkbench}
          >
            <Settings size={18} />
          </button>
        </div>

        <footer className="mini-footer">
          <span className={`status-pill ${status}`}>
            <span />
            {statusLabel}
          </span>
          <span className="hotkey-hint">
            紧急停止: Ctrl + Alt + S
            <Keyboard size={14} />
          </span>
        </footer>

        {recordingWarningVisible ? (
          <div className="recording-warning-popover" role="alert">
            <span>{recordingSafetyWarning}</span>
            <div>
              <button className="toolbar-button slim" onClick={onCancelRecordingStart}>
                取消
              </button>
              <button
                className="toolbar-button primary slim"
                onClick={onConfirmRecordingStart}
              >
                继续录制
              </button>
            </div>
          </div>
        ) : null}
      </section>
    </main>
  );
}
