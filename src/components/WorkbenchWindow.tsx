import {
  AlertCircle,
  Clock3,
  Crosshair,
  Keyboard,
  ListChecks,
  Play,
  Plus,
  Save,
  SlidersHorizontal,
  Trash2,
} from "lucide-react";
import type { AppStatus, Flow, FlowStep, SaveStatus } from "../types";
import { findLatestSafetyStopLog, type RunLogEntry } from "../runLog";
import { parseSpeedMultiplier } from "../flowTiming";

interface WorkbenchWindowProps {
  flow: Flow;
  selectedFileName: string;
  savedAt: number;
  saveStatus: SaveStatus;
  saveMessage: string;
  isFlowLoading: boolean;
  targetSafetyRunId: number | null;
  selectedStepId: number | null;
  runLogs: RunLogEntry[];
  status: AppStatus;
  statusLabel: string;
  loopCount: number;
  speed: string;
  onLoopCountChange: (value: number) => void;
  onSpeedChange: (value: string) => void;
  onStatusChange: (status: AppStatus) => void;
  onFlowDisplayNameChange: (displayName: string) => void;
  onStepSelect: (stepId: number) => void;
  onStepDelayChange: (stepId: number, delayMs: number) => void;
  onStepClickCoordinatesChange: (stepId: number, x: number, y: number) => void;
  onStepTextChange: (stepId: number, text: string) => void;
  onStepHotkeyChange: (stepId: number, hotkeyText: string) => void;
  onStepKeyChange: (stepId: number, keyText: string) => void;
  onTargetWindowMatchedChange: (matched: boolean) => void;
  onStepDelete: (stepId: number) => void;
  onInsertWaitStep: () => void;
  onInsertTypeStep: () => void;
  onInsertHotkeyStep: () => void;
  onInsertKeyStep: () => void;
  onSaveFlow: () => void;
  onSaveFlowAs: () => void;
  onEmergencyStop: () => void;
  emergencyStopHint: string;
  infiniteLoopWarningVisible: boolean;
  infiniteLoopConfirmed: boolean;
  infiniteLoopWarning: string;
  onConfirmInfiniteLoop: () => void;
  onCancelInfiniteLoop: () => void;
}

function stepLabel(step: FlowStep) {
  if (step.type === "click") return "Click";
  if (step.type === "drag") return "Drag";
  if (step.type === "type") return "Type";
  if (step.type === "key") return "Key";
  if (step.type === "wait") return "Wait";
  if (step.type === "scroll") return "Scroll";
  return "Hotkey";
}

function stepValue(step: FlowStep) {
  if (step.type === "click") return step.target;
  if (step.type === "drag") return step.target;
  if (step.type === "type") return step.text;
  if (step.type === "key") return step.key;
  if (step.type === "wait") return `${(step.durationMs / 1000).toFixed(1)}s`;
  if (step.type === "scroll") {
    const position =
      typeof step.x === "number" && typeof step.y === "number"
        ? `(${step.x}, ${step.y})`
        : "当前位置";
    return `${position} · X ${step.deltaX}, Y ${step.deltaY}`;
  }
  return step.keys.join(" + ");
}

function stepMode(step: FlowStep) {
  if (step.type === "click") return "屏幕坐标";
  if (step.type === "drag") return "拖拽轨迹";
  if (step.type === "type") return "文本输入";
  if (step.type === "key") return "普通按键";
  if (step.type === "wait") return "等待时长";
  if (step.type === "scroll") return "滚轮增量";
  return "快捷键组合";
}

function StepIcon({ step }: { step: FlowStep }) {
  if (step.type === "type") return <span className="step-letter">T</span>;
  if (step.type === "key") return <Keyboard size={16} />;
  if (step.type === "wait") return <Clock3 size={16} />;
  if (step.type === "hotkey") return <Keyboard size={16} />;
  if (step.type === "scroll") return <SlidersHorizontal size={16} />;
  return <Crosshair size={16} />;
}

function formatRunLogTime(time: number) {
  return new Date(time).toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function formatDelaySeconds(delayMs: number) {
  return (delayMs / 1000).toFixed(2).replace(/\.?0+$/, "");
}

function stepDurationMs(step: FlowStep) {
  if (step.type === "drag") return step.delayMs + step.durationMs;
  return step.type === "wait" ? step.durationMs : step.delayMs;
}

function formatEstimatedDuration(flow: Flow, speed: string, loopCount: number) {
  if (loopCount === 0) return "直到停止";

  const safeLoopCount = Number.isFinite(loopCount) && loopCount > 0 ? loopCount : 1;
  const totalMs =
    flow.steps.reduce((total, step) => total + stepDurationMs(step), 0) *
    safeLoopCount /
    parseSpeedMultiplier(speed);
  const totalSeconds = Math.ceil(totalMs / 1000);
  const minutes = Math.floor(totalSeconds / 60).toString().padStart(2, "0");
  const seconds = (totalSeconds % 60).toString().padStart(2, "0");
  return `${minutes}:${seconds}`;
}

export function WorkbenchWindow({
  flow,
  selectedFileName,
  savedAt,
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
  onLoopCountChange,
  onSpeedChange,
  onStatusChange,
  onFlowDisplayNameChange,
  onStepSelect,
  onStepDelayChange,
  onStepClickCoordinatesChange,
  onStepTextChange,
  onStepHotkeyChange,
  onStepKeyChange,
  onTargetWindowMatchedChange,
  onStepDelete,
  onInsertWaitStep,
  onInsertTypeStep,
  onInsertHotkeyStep,
  onInsertKeyStep,
  onSaveFlow,
  onSaveFlowAs,
  onEmergencyStop,
  emergencyStopHint,
  infiniteLoopWarningVisible,
  infiniteLoopConfirmed,
  infiniteLoopWarning,
  onConfirmInfiniteLoop,
  onCancelInfiniteLoop,
}: WorkbenchWindowProps) {
  const selectedStep =
    flow.steps.find((step) => step.id === selectedStepId) ?? flow.steps[0];
  const activeSelectedStepId = selectedStep?.id ?? null;
  const targetSafetyStop =
    targetSafetyRunId === null
      ? undefined
      : findLatestSafetyStopLog(runLogs, flow.displayName, targetSafetyRunId);
  const targetState = targetSafetyStop
    ? "safety-stopped"
    : flow.targetWindow.matched
      ? "matched"
      : "unmatched";
  const targetStateLabel = targetSafetyStop
    ? "安全停止"
    : flow.targetWindow.matched
      ? "已匹配"
      : "未匹配";
  const targetStateDetail =
    targetSafetyStop?.detail ??
    (flow.targetWindow.matched
      ? "回放前会检查目标窗口进程和标题，避免输入到明显错误的窗口。"
      : "目标窗口尚未确认，点击、拖拽、文本、按键、热键和滚轮回放会被拒绝。");
  const estimatedDuration = formatEstimatedDuration(flow, speed, loopCount);

  function handleSelectedDelayChange(value: string) {
    if (!selectedStep) return;
    const nextSeconds = Number(value);
    if (!Number.isFinite(nextSeconds)) return;
    onStepDelayChange(selectedStep.id, nextSeconds * 1000);
  }

  function handleSelectedClickCoordinateChange(axis: "x" | "y", value: string) {
    if (selectedStep?.type !== "click") return;
    const nextValue = Number(value);
    if (!Number.isFinite(nextValue)) return;
    onStepClickCoordinatesChange(
      selectedStep.id,
      axis === "x" ? nextValue : selectedStep.x,
      axis === "y" ? nextValue : selectedStep.y,
    );
  }

  return (
    <main className="workbench-window">
      <div className="workbench-shell">
        <div className="workbench-grid">
          <aside className="sidebar">
            <button className="sidebar-item active">
              <ListChecks size={18} />
              <span>流程编辑</span>
            </button>
          </aside>

          <section className="workspace">
            <div className="workspace-header">
              <div className="breadcrumb">
                <span>流程编辑</span>
                <span>›</span>
                <strong>{flow.displayName}</strong>
                <span className={`save-indicator ${saveStatus}`}>
                  {saveMessage}
                </span>
              </div>

              <div className="command-bar">
                <button
                  className="toolbar-button"
                  disabled={isFlowLoading || saveStatus === "saving"}
                  onClick={onSaveFlow}
                >
                  <Save size={16} />
                  {saveStatus === "saving" ? "保存中" : "保存流程"}
                </button>
                <button
                  className="toolbar-button"
                  disabled={isFlowLoading || saveStatus === "saving"}
                  onClick={onSaveFlowAs}
                >
                  <Save size={16} />
                  另存为
                </button>
                <button
                  className="toolbar-button primary"
                  disabled={
                    isFlowLoading || status === "recording" || status === "playing"
                  }
                  onClick={() => onStatusChange("playing")}
                >
                  <Play size={16} fill="currentColor" />
                  运行
                </button>
                <button
                  className="toolbar-button danger"
                  disabled={status !== "playing"}
                  onClick={onEmergencyStop}
                >
                  <AlertCircle size={16} />
                  紧急停止
                </button>
              </div>
            </div>

            {infiniteLoopWarningVisible ? (
              <div className="workbench-warning-bar" role="alert">
                <span>{infiniteLoopWarning}</span>
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
            ) : null}

            <div className="flow-timeline">
              {flow.steps.map((step) => (
                <button
                  className={`timeline-node ${step.type} ${
                    step.id === activeSelectedStepId ? "active" : ""
                  }`}
                  key={step.id}
                  onClick={() => onStepSelect(step.id)}
                  type="button"
                >
                  <div className="node-head">
                    <StepIcon step={step} />
                    <strong>{stepLabel(step)}</strong>
                    <span>{(step.delayMs / 1000).toFixed(1)}s</span>
                  </div>
                  <span>{step.note}</span>
                </button>
              ))}
              <button className="add-node" onClick={onInsertWaitStep}>
                <Plus size={16} />
                添加等待
              </button>
            </div>

            <div className="content-grid">
              <section className="steps-panel">
                <div className="tabs-row">
                  <button className="tab active">步骤 ({flow.steps.length})</button>
                  <div className="insert-actions">
                    <button className="toolbar-button slim" onClick={onInsertWaitStep}>
                      <Plus size={15} />
                      等待
                    </button>
                    <button className="toolbar-button slim" onClick={onInsertTypeStep}>
                      <Plus size={15} />
                      文本
                    </button>
                    <button className="toolbar-button slim" onClick={onInsertHotkeyStep}>
                      <Plus size={15} />
                      快捷键
                    </button>
                    <button className="toolbar-button slim" onClick={onInsertKeyStep}>
                      <Plus size={15} />
                      按键
                    </button>
                  </div>
                  <button
                    className="toolbar-button danger slim"
                    disabled={!selectedStep}
                    onClick={() => selectedStep && onStepDelete(selectedStep.id)}
                  >
                    <Trash2 size={15} />
                    删除步骤
                  </button>
                </div>

                <table className="steps-table">
                  <thead>
                    <tr>
                      <th>#</th>
                      <th>类型</th>
                      <th>操作</th>
                      <th>目标 / 值</th>
                      <th>延迟</th>
                      <th>备注</th>
                    </tr>
                  </thead>
                  <tbody>
                    {flow.steps.map((step) => (
                      <tr
                        className={step.id === activeSelectedStepId ? "selected" : ""}
                        key={step.id}
                        onClick={() => onStepSelect(step.id)}
                      >
                        <td>{step.id}</td>
                        <td>
                          <span className={`table-kind ${step.type}`}>
                            <StepIcon step={step} />
                            {stepLabel(step)}
                          </span>
                        </td>
                        <td>{step.action}</td>
                        <td>{stepValue(step)}</td>
                        <td>{(step.delayMs / 1000).toFixed(1)}s</td>
                        <td>{step.note}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </section>

              <aside className="inspector">
                <div className="tabs-row inspector-tabs">
                  <button className="tab active">步骤设置</button>
                </div>

                <label className="form-control">
                  <span>流程名称</span>
                  <input
                    value={flow.displayName}
                    onChange={(event) =>
                      onFlowDisplayNameChange(event.target.value)
                    }
                  />
                </label>

                <label className="form-control">
                  <span>步骤类型</span>
                  <span className="readonly-value">
                    {selectedStep
                      ? `${stepLabel(selectedStep)} - ${selectedStep.action}`
                      : "未选择"}
                  </span>
                </label>

                <label className="form-control">
                  <span>方式</span>
                  <span className="readonly-value">
                    {selectedStep ? stepMode(selectedStep) : "未选择"}
                  </span>
                </label>

                {selectedStep?.type === "click" ? (
                  <div className="coordinate-row">
                    <label>
                      X
                      <input
                        aria-label="点击 X 坐标"
                        onChange={(event) =>
                          handleSelectedClickCoordinateChange("x", event.target.value)
                        }
                        type="number"
                        value={selectedStep.x}
                      />
                    </label>
                    <label>
                      Y
                      <input
                        aria-label="点击 Y 坐标"
                        onChange={(event) =>
                          handleSelectedClickCoordinateChange("y", event.target.value)
                        }
                        type="number"
                        value={selectedStep.y}
                      />
                    </label>
                  </div>
                ) : null}

                {selectedStep?.type === "drag" ? (
                  <div className="drag-coordinate-grid">
                    <label>
                      起点 X
                      <input readOnly value={selectedStep.startX} />
                    </label>
                    <label>
                      起点 Y
                      <input readOnly value={selectedStep.startY} />
                    </label>
                    <label>
                      终点 X
                      <input readOnly value={selectedStep.endX} />
                    </label>
                    <label>
                      终点 Y
                      <input readOnly value={selectedStep.endY} />
                    </label>
                  </div>
                ) : null}

                <label className="form-control">
                  <span>延迟</span>
                  <input
                    aria-label="步骤延迟秒"
                    disabled={!selectedStep}
                    min="0"
                    onChange={(event) => handleSelectedDelayChange(event.target.value)}
                    step="0.1"
                    type="number"
                    value={selectedStep ? formatDelaySeconds(selectedStep.delayMs) : ""}
                  />
                </label>

                {selectedStep?.type === "type" ? (
                  <label className="form-control">
                    <span>输入文本</span>
                    <input
                      aria-label="步骤输入文本"
                      onChange={(event) =>
                        onStepTextChange(selectedStep.id, event.target.value)
                      }
                      value={selectedStep.text}
                    />
                  </label>
                ) : null}

                {selectedStep?.type === "hotkey" ? (
                  <label className="form-control">
                    <span>快捷键</span>
                    <input
                      aria-label="步骤快捷键"
                      onChange={(event) =>
                        onStepHotkeyChange(selectedStep.id, event.target.value)
                      }
                      value={selectedStep.keys.join(" + ")}
                    />
                  </label>
                ) : null}

                {selectedStep?.type === "key" ? (
                  <label className="form-control">
                    <span>按键</span>
                    <input
                      aria-label="步骤按键"
                      onChange={(event) =>
                        onStepKeyChange(selectedStep.id, event.target.value)
                      }
                      value={selectedStep.key}
                    />
                  </label>
                ) : null}

                <label className="form-control">
                  <span>备注</span>
                  <input value={selectedStep?.note ?? ""} readOnly />
                </label>

                <div className="checkbox-group">
                  <strong>安全与控制</strong>
                  <label>
                    <input
                      checked={flow.targetWindow.matched}
                      onChange={(event) =>
                        onTargetWindowMatchedChange(event.target.checked)
                      }
                      type="checkbox"
                    />
                    目标窗口已确认
                  </label>
                </div>

                <div className="run-controls">
                  <span>运行控制</span>
                  <div className="segmented">
                    {["0.5x", "1x", "2x", "5x"].map((item) => (
                      <button
                        className={speed === item ? "active" : ""}
                        disabled={isFlowLoading}
                        key={item}
                        onClick={() => onSpeedChange(item)}
                      >
                        {item}
                      </button>
                    ))}
                  </div>
                  <label className="loop-control">
                    循环次数
                    <select
                      aria-label="循环次数"
                      disabled={isFlowLoading}
                      value={loopCount}
                      onChange={(event) =>
                        onLoopCountChange(Number(event.target.value))
                      }
                    >
                      <option value={1}>1 次</option>
                      <option value={3}>3 次</option>
                      <option value={10}>10 次</option>
                      <option value={0}>无限循环</option>
                    </select>
                    <span
                      className={
                        loopCount === 0 && infiniteLoopConfirmed
                          ? "loop-confirmed"
                          : "loop-disabled"
                      }
                    >
                      {loopCount === 0 && infiniteLoopConfirmed
                        ? "已二次确认"
                        : "无限循环需二次确认"}
                    </span>
                  </label>
                  <button
                    className="emergency-button"
                    disabled={status !== "playing"}
                    onClick={onEmergencyStop}
                  >
                    <AlertCircle size={17} />
                    紧急停止
                  </button>
                </div>
              </aside>
            </div>

            <footer className="bottom-grid">
              <section className="target-preview">
                <span>目标窗口</span>
                <div className="preview-body">
                  <div className="target-window-summary">
                    <strong>{flow.targetWindow.title}</strong>
                    <p>{flow.targetWindow.process}</p>
                    <p>{flow.targetWindow.size}</p>
                    <em className={targetState}>{targetStateLabel}</em>
                    <p className={`target-safety-detail ${targetState}`}>
                      {targetStateDetail}
                    </p>
                  </div>
                </div>
              </section>

              <section className="run-log-panel" aria-label="运行日志">
                <div className="section-heading-row">
                  <span>运行日志</span>
                  <em>{runLogs.length ? `${runLogs.length} 条` : "空"}</em>
                </div>
                <div className="run-log-list" role="log" aria-live="polite">
                  {runLogs.length ? (
                    runLogs.map((entry) => (
                      <div className={`run-log-entry ${entry.level}`} key={entry.id}>
                        <span>{formatRunLogTime(entry.time)}</span>
                        <strong>{entry.title}</strong>
                        <p>{entry.detail}</p>
                        <em>
                          {entry.flowName}
                          {entry.completedSteps !== undefined
                            ? ` · ${entry.completedSteps} 执行 / ${entry.skippedSteps ?? 0} 跳过`
                            : ""}
                        </em>
                      </div>
                    ))
                  ) : (
                    <p className="empty-run-log">暂无运行记录</p>
                  )}
                </div>
              </section>

              <section className="status-panel">
                <span>当前状态</span>
                <strong>{statusLabel}</strong>
                <p>步骤: {flow.steps.length}</p>
                <p>文件: {selectedFileName}</p>
                <p>保存: {savedAt ? saveMessage.replace(/^已保存: /, "") : "尚未保存"}</p>
                <p>预计用时: {estimatedDuration}</p>
                <p>{emergencyStopHint}</p>
              </section>
            </footer>
          </section>
        </div>
      </div>
    </main>
  );
}
