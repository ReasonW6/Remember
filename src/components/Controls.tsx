import { Circle, FolderOpen, Play, Save, Square } from "lucide-react";
import type { UiState } from "../types";

interface ControlsProps {
  state: UiState;
  hasRecording: boolean;
  onRecord: () => void;
  onPlay: () => void;
  onStop: () => void;
  onSave: () => void;
  onOpen: () => void;
}

export function Controls({
  state,
  hasRecording,
  onRecord,
  onPlay,
  onStop,
  onSave,
  onOpen
}: ControlsProps) {
  const isRecording = state.mode === "recording";
  const isPlaying = state.mode === "playing";
  const isBusy = isRecording || isPlaying;

  return (
    <section className="panel controls-panel" aria-label="Controls">
      <button
        className="action-button"
        type="button"
        onClick={onRecord}
        disabled={isPlaying}
      >
        <Circle size={16} aria-hidden="true" />
        <span className="button-label">{isRecording ? "Stop recording" : "Record"}</span>
      </button>
      <button
        className="action-button"
        type="button"
        onClick={onPlay}
        disabled={!hasRecording || isBusy}
      >
        <Play size={16} aria-hidden="true" />
        <span className="button-label">Play</span>
      </button>
      <button
        className="action-button"
        type="button"
        onClick={onStop}
        disabled={!isBusy}
      >
        <Square size={16} aria-hidden="true" />
        <span className="button-label">Stop</span>
      </button>
      <button
        className="action-button"
        type="button"
        onClick={onSave}
        disabled={!hasRecording || isBusy}
      >
        <Save size={16} aria-hidden="true" />
        <span className="button-label">Save</span>
      </button>
      <button
        className="action-button"
        type="button"
        onClick={onOpen}
        disabled={isBusy}
      >
        <FolderOpen size={16} aria-hidden="true" />
        <span className="button-label">Open</span>
      </button>
    </section>
  );
}
