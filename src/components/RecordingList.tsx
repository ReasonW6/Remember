import { useState, type FormEvent } from "react";
import { Check, Pencil, RefreshCw, X } from "lucide-react";
import { displayErrorMessage } from "../localization";
import type { RecordingFile } from "../types";

interface RecordingListProps {
  recordings: RecordingFile[];
  selectedPath: string | null;
  disabled: boolean;
  onSelect: (path: string) => void;
  onDelete: (recording: RecordingFile, force: boolean) => void;
  onRename: (recording: RecordingFile, newName: string) => void;
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
  onRename,
  onRefresh
}: RecordingListProps) {
  const [editingPath, setEditingPath] = useState<string | null>(null);
  const [draftName, setDraftName] = useState("");

  function startRenaming(recording: RecordingFile) {
    setEditingPath(recording.path);
    setDraftName(recording.name);
  }

  function cancelRenaming() {
    setEditingPath(null);
    setDraftName("");
  }

  function submitRename(event: FormEvent, recording: RecordingFile) {
    event.preventDefault();
    const newName = draftName.trim();
    if (newName && newName !== recording.name) {
      onRename(recording, newName);
    }
    cancelRenaming();
  }

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
                {editingPath === recording.path ? (
                  <form
                    className="recording-rename-form"
                    onSubmit={(event) => submitRename(event, recording)}
                  >
                    <input
                      className="recording-rename-input"
                      aria-label={`重命名 ${recording.name}`}
                      value={draftName}
                      onChange={(event) => setDraftName(event.currentTarget.value)}
                      onFocus={(event) => event.currentTarget.select()}
                      onKeyDown={(event) => {
                        if (event.key === "Escape") {
                          event.preventDefault();
                          cancelRenaming();
                        }
                      }}
                      autoFocus
                      required
                    />
                    <button
                      className="recording-rename-action"
                      type="submit"
                      aria-label={`保存 ${recording.name} 的新名称`}
                      disabled={!draftName.trim()}
                    >
                      <Check size={15} aria-hidden="true" />
                    </button>
                    <button
                      className="recording-rename-action"
                      type="button"
                      aria-label="取消重命名"
                      onClick={cancelRenaming}
                    >
                      <X size={15} aria-hidden="true" />
                    </button>
                  </form>
                ) : (
                  <>
                    <button
                      className="recording-rename-button"
                      type="button"
                      aria-label={`重命名 ${recording.name}`}
                      onClick={() => startRenaming(recording)}
                      disabled={disabled || Boolean(recording.load_error)}
                    >
                      <Pencil size={14} aria-hidden="true" />
                    </button>
                    <button
                      className={`recording-item ${selectedPath === recording.path ? "selected" : ""} ${recording.load_error ? "invalid" : ""}`}
                      type="button"
                      aria-label={
                        recording.load_error
                          ? `无法载入 ${recording.name}`
                          : `选择 ${recording.name}`
                      }
                      aria-pressed={selectedPath === recording.path}
                      onClick={() => onSelect(recording.path)}
                      disabled={disabled || Boolean(recording.load_error)}
                    >
                      <span className="recording-name">{recording.name}</span>
                      <span className="recording-meta">
                        {recording.step_count} 步 · {recording.duration_ms} ms
                        {formatUpdatedTime(recording.updated_at_ms)
                          ? ` · ${formatUpdatedTime(recording.updated_at_ms)}`
                          : ""}
                      </span>
                      {recording.load_error ? (
                        <span className="recording-load-error">
                          {displayErrorMessage(recording.load_error)}
                        </span>
                      ) : null}
                    </button>
                    <button
                      className="recording-delete-button"
                      type="button"
                      aria-label={`删除 ${recording.name}`}
                      data-tooltip="按住 Ctrl 点击强制删除"
                      onClick={(event) => onDelete(recording, event.ctrlKey)}
                      disabled={disabled}
                    >
                      <X size={15} aria-hidden="true" />
                    </button>
                  </>
                )}
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
