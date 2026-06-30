import type { UiState } from "../types";

interface StatusPanelProps {
  state: UiState;
  error: string;
}

export function StatusPanel({ state, error }: StatusPanelProps) {
  return (
    <section className="panel status-panel" aria-labelledby="status-title">
      <div className="section-heading">
        <h2 id="status-title">Status</h2>
        <span className={`mode-pill mode-${state.mode}`}>{state.mode}</span>
      </div>
      {error ? <p className="status-message alert">{error}</p> : null}
      <dl className="status-list">
        <div>
          <dt>Message</dt>
          <dd>{state.message}</dd>
        </div>
        <div>
          <dt>Recording</dt>
          <dd>{state.recording_name ?? "None"}</dd>
        </div>
        <div>
          <dt>Steps</dt>
          <dd>{state.step_count}</dd>
        </div>
        <div>
          <dt>Duration</dt>
          <dd>{state.duration_ms} ms</dd>
        </div>
      </dl>
    </section>
  );
}
