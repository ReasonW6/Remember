import { Circle, FolderOpen, Play, Save, Square } from "lucide-react";
import type { UiState } from "../types";

interface ControlsProps {
  state: UiState;
  hasRecording: boolean;
  pendingCommand: boolean;
  onRecord: () => void;
  onPlay: () => void;
  onStop: () => void;
  onSave: () => void;
  onOpen: () => void;
}

export function Controls({
  state,
  hasRecording,
  pendingCommand,
  onRecord,
  onPlay,
  onStop,
  onSave,
  onOpen
}: ControlsProps) {
  const isRecording = state.mode === "recording";
  const isPlaying = state.mode === "playing";
  const isBusy = isRecording || isPlaying;
  const RecordStopIcon = isBusy ? Square : Circle;

  return (
    <section className="panel controls-panel" aria-label="控制">
      <button
        className="action-button"
        type="button"
        onClick={isBusy ? onStop : onRecord}
        disabled={pendingCommand}
      >
        <RecordStopIcon size={16} aria-hidden="true" />
        <span className="button-label">{isBusy ? "停止" : "录制"}</span>
      </button>
      <button
        className="action-button"
        type="button"
        onClick={onPlay}
        disabled={pendingCommand || !hasRecording || isBusy}
      >
        <Play size={16} aria-hidden="true" />
        <span className="button-label">播放</span>
      </button>
      <button
        className="action-button"
        type="button"
        onClick={onSave}
        disabled={pendingCommand || !hasRecording || isBusy}
      >
        <Save size={16} aria-hidden="true" />
        <span className="button-label">保存</span>
      </button>
      <button
        className="action-button"
        type="button"
        onClick={onOpen}
        disabled={pendingCommand || isBusy}
      >
        <FolderOpen size={16} aria-hidden="true" />
        <span className="button-label">打开</span>
      </button>
    </section>
  );
}
