import { Circle, Play, Shield, ShieldCheck, Square } from "lucide-react";
import type { RecordingFile, UiState } from "../types";

interface CompactControlsProps {
  state: UiState;
  recordings: RecordingFile[];
  selectedPath: string | null;
  selectedName: string | null;
  hasRecording: boolean;
  playbackValid: boolean;
  pendingCommand: boolean;
  isElevated: boolean;
  message: string;
  error: string;
  onSelect: (path: string) => void;
  onRecord: () => void;
  onPlay: () => void;
  onStop: () => void;
  onRestartAsAdministrator: () => void;
}

export function CompactControls({
  state,
  recordings,
  selectedPath,
  selectedName,
  hasRecording,
  playbackValid,
  pendingCommand,
  isElevated,
  message,
  error,
  onSelect,
  onRecord,
  onPlay,
  onStop,
  onRestartAsAdministrator
}: CompactControlsProps) {
  const isRecording = state.mode === "recording";
  const isPlaying = state.mode === "playing";
  const availableRecordings = recordings.filter((recording) => !recording.load_error);
  const selectedIsExternal =
    selectedPath !== null &&
    !availableRecordings.some((recording) => recording.path === selectedPath);

  return (
    <section className="compact-content" aria-label="快速控制">
      <div className="compact-controls">
        <label className="sr-only" htmlFor="compact-recording-select">
          选择录制文件
        </label>
        <select
          id="compact-recording-select"
          className="compact-recording-select"
          value={selectedPath ?? ""}
          onChange={(event) => {
            if (event.currentTarget.value) {
              onSelect(event.currentTarget.value);
            }
          }}
          disabled={
            pendingCommand ||
            isRecording ||
            isPlaying ||
            (availableRecordings.length === 0 && !selectedIsExternal)
          }
        >
          {selectedPath === null ? (
            <option value="" disabled>
              {availableRecordings.length === 0 ? "暂无录制文件" : "选择录制文件"}
            </option>
          ) : null}
          {selectedIsExternal ? (
            <option value={selectedPath}>{selectedName || "外部录制文件"}</option>
          ) : null}
          {availableRecordings.map((recording) => (
            <option key={recording.path} value={recording.path}>
              {recording.name}
            </option>
          ))}
        </select>
        <button
          id="compact-record-button"
          className="action-button compact-action-button"
          type="button"
          onClick={isRecording ? onStop : onRecord}
          disabled={pendingCommand || isPlaying}
        >
          {isRecording ? (
            <Square size={14} aria-hidden="true" />
          ) : (
            <Circle size={14} aria-hidden="true" />
          )}
          <span>{isRecording ? "停止" : "录制"}</span>
        </button>
        <button
          className="action-button compact-action-button"
          type="button"
          onClick={isPlaying ? onStop : onPlay}
          disabled={
            pendingCommand || isRecording || (!isPlaying && (!hasRecording || !playbackValid))
          }
        >
          {isPlaying ? (
            <Square size={14} aria-hidden="true" />
          ) : (
            <Play size={14} aria-hidden="true" />
          )}
          <span>{isPlaying ? "停止" : "播放"}</span>
        </button>
        <button
          className="action-button compact-action-button compact-admin-button"
          type="button"
          aria-label={isElevated ? "已在管理员模式" : "以管理员身份重启"}
          title={isElevated ? "已在管理员模式" : "以管理员身份重启"}
          onClick={onRestartAsAdministrator}
          disabled={pendingCommand || isRecording || isPlaying || isElevated}
        >
          {isElevated ? (
            <ShieldCheck size={14} aria-hidden="true" />
          ) : (
            <Shield size={14} aria-hidden="true" />
          )}
        </button>
      </div>
      <p
        className={error ? "compact-feedback compact-feedback-error" : "compact-feedback"}
        role={error ? "alert" : "status"}
        aria-live="polite"
      >
        {error || message}
      </p>
    </section>
  );
}
