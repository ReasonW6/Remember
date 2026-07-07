import { RefreshCw, X } from "lucide-react";
import type { RecordingFile } from "../types";

interface RecordingListProps {
  recordings: RecordingFile[];
  selectedPath: string | null;
  disabled: boolean;
  onSelect: (path: string) => void;
  onDelete: (path: string) => void;
  onRefresh: () => void;
}

function formatUpdatedTime(updatedAtMs: number) {
  if (!updatedAtMs) {
    return "";
  }
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit"
  }).format(new Date(updatedAtMs));
}

export function RecordingList({
  recordings,
  selectedPath,
  disabled,
  onSelect,
  onDelete,
  onRefresh
}: RecordingListProps) {
  return (
    <section className="panel recording-list-panel" aria-labelledby="recordings-title">
      <div className="section-heading">
        <h2 id="recordings-title">录制文件</h2>
        <button
          className="icon-button"
          type="button"
          aria-label="刷新录制文件"
          onClick={onRefresh}
          disabled={disabled}
        >
          <RefreshCw size={15} aria-hidden="true" />
        </button>
      </div>
      {recordings.length === 0 ? (
        <p className="empty-text">暂无录制文件</p>
      ) : (
        <ul className="recording-list">
          {recordings.map((recording) => (
            <li key={recording.path}>
              <div className="recording-row">
                <button
                  className={`recording-item ${selectedPath === recording.path ? "selected" : ""}`}
                  type="button"
                  aria-label={`选择 ${recording.name}`}
                  onClick={() => onSelect(recording.path)}
                  disabled={disabled}
                >
                  <span className="recording-name">{recording.name}</span>
                  <span className="recording-meta">
                    {recording.step_count} 步 · {recording.duration_ms} ms
                    {formatUpdatedTime(recording.updated_at_ms)
                      ? ` · ${formatUpdatedTime(recording.updated_at_ms)}`
                      : ""}
                  </span>
                </button>
                <button
                  className="recording-delete-button"
                  type="button"
                  aria-label={`删除 ${recording.name}`}
                  onClick={() => onDelete(recording.path)}
                  disabled={disabled}
                >
                  <X size={15} aria-hidden="true" />
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
