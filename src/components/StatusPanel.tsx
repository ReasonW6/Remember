import type { UiState } from "../types";
import { displayMessage, displayMode } from "../localization";

interface StatusPanelProps {
  state: UiState;
  error: string;
}

export function StatusPanel({ state, error }: StatusPanelProps) {
  return (
    <section className="panel status-panel" aria-labelledby="status-title">
      <div className="section-heading">
        <h2 id="status-title">状态</h2>
        <span className={`mode-pill mode-${state.mode}`}>{displayMode(state.mode)}</span>
      </div>
      {error ? (
        <p className="status-message alert" role="alert">
          {error}
        </p>
      ) : null}
      <dl className="status-list">
        <div>
          <dt>消息</dt>
          <dd>{displayMessage(state.message)}</dd>
        </div>
        <div>
          <dt>录制文件</dt>
          <dd>{state.recording_name ?? "无"}</dd>
        </div>
        <div>
          <dt>步骤数</dt>
          <dd>{state.step_count}</dd>
        </div>
        <div>
          <dt>时长</dt>
          <dd>{state.duration_ms} ms</dd>
        </div>
      </dl>
    </section>
  );
}
