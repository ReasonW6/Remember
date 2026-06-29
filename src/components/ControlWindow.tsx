import {
  Circle,
  Play,
  Settings,
  Square,
  Keyboard,
} from "lucide-react";
import { buildFlowOptions } from "../flowOptions";
import type { AppStatus, Flow, FlowSummary, SavedFlow } from "../types";

interface ControlWindowProps {
  flow: Flow;
  flowSummaries: FlowSummary[];
  retainedDraft: SavedFlow | null;
  selectedFileName: string;
  status: AppStatus;
  statusLabel: string;
  loopCount: number;
  speed: string;
  isFlowLoading: boolean;
  onLoopCountChange: (value: number) => void;
  onSpeedChange: (value: string) => void;
  onStatusChange: (status: AppStatus) => void;
  onFlowSelect: (fileName: string) => void;
  onOpenWorkbench: () => void;
  emergencyStopHint: string;
  infiniteLoopWarningVisible: boolean;
  infiniteLoopConfirmed: boolean;
  infiniteLoopWarning: string;
  onConfirmInfiniteLoop: () => void;
  onCancelInfiniteLoop: () => void;
}

export function ControlWindow({
  flow,
  flowSummaries,
  retainedDraft,
  selectedFileName,
  status,
  statusLabel,
  loopCount,
  speed,
  isFlowLoading,
  onLoopCountChange,
  onSpeedChange,
  onStatusChange,
  onFlowSelect,
  onOpenWorkbench,
  emergencyStopHint,
  infiniteLoopWarningVisible,
  infiniteLoopConfirmed,
  infiniteLoopWarning,
  onConfirmInfiniteLoop,
  onCancelInfiniteLoop,
}: ControlWindowProps) {
  const flowOptions = buildFlowOptions({
    flow,
    flowSummaries,
    selectedFileName,
    retainedDraft,
  });
  const invalidFlowSummaries = flowOptions.filter((summary) => !summary.isValid);
  const invalidFlowDetail = invalidFlowSummaries
    .map((summary) => `${summary.displayName}: ${summary.error ?? "未知错误"}`)
    .join("\n");

  return (
    <main className="control-window">
      <section className="control-card">
        <div className="mini-body">
          <label className="field flow-field">
            <span>当前流程</span>
            <select
              aria-label="当前流程"
              disabled={isFlowLoading || infiniteLoopWarningVisible}
              value={selectedFileName}
              onChange={(event) => onFlowSelect(event.target.value)}
            >
              {flowOptions.map((summary) => (
                <option
                  disabled={!summary.isValid}
                  value={summary.fileName}
                  key={summary.fileName}
                  title={summary.error ?? undefined}
                >
                  {summary.isValid ? summary.displayName : `${summary.displayName} - 无法加载`}
                </option>
              ))}
            </select>
          </label>

          <button
            className="action-button record"
            disabled={
              status === "recording" ||
              isFlowLoading ||
              infiniteLoopWarningVisible
            }
            onClick={() => onStatusChange("recording")}
          >
            <Circle size={17} fill="currentColor" />
            <span>录制</span>
          </button>
          <button
            className="action-button replay"
            disabled={
              infiniteLoopWarningVisible ||
              isFlowLoading ||
              status === "recording" ||
              status === "playing"
            }
            onClick={() => onStatusChange("playing")}
          >
            <Play size={18} fill="currentColor" />
            <span>重放</span>
          </button>
          <button
            className="action-button stop"
            disabled={
              infiniteLoopWarningVisible ||
              isFlowLoading ||
              status === "ready" ||
              status === "stopped"
            }
            onClick={() => onStatusChange("stopped")}
          >
            <Square size={15} fill="currentColor" />
            <span>停止</span>
          </button>

          <label className="field tiny-field">
            <span>速度</span>
            <select
              aria-label="速度"
              disabled={isFlowLoading}
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
              disabled={isFlowLoading}
              value={loopCount}
              onChange={(event) => onLoopCountChange(Number(event.target.value))}
            >
              <option value={1}>1</option>
              <option value={3}>3</option>
              <option value={10}>10</option>
              <option value={0}>∞</option>
            </select>
          </label>

          <button
            className="icon-button settings-button"
            aria-label="设置"
            disabled={isFlowLoading || infiniteLoopWarningVisible}
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
            {invalidFlowSummaries.length
              ? `${invalidFlowSummaries.length} 个流程无法加载`
              : loopCount === 0 && infiniteLoopConfirmed
                ? "无限循环已确认"
                : emergencyStopHint}
            <Keyboard size={14} />
          </span>
          {invalidFlowSummaries.length ? (
            <span className="invalid-flow-detail" title={invalidFlowDetail}>
              查看错误
            </span>
          ) : null}
        </footer>

        {infiniteLoopWarningVisible ? (
          <div className="recording-warning-popover infinite-loop-popover" role="alert">
            <span>{infiniteLoopWarning}</span>
            <div>
              <button className="toolbar-button slim" onClick={onCancelInfiniteLoop}>
                取消
              </button>
              <button
                className="toolbar-button primary slim"
                onClick={onConfirmInfiniteLoop}
              >
                确认无限循环
              </button>
            </div>
          </div>
        ) : null}
      </section>
    </main>
  );
}
