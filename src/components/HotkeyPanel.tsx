import { Save } from "lucide-react";
import { type KeyboardEvent, useEffect, useState } from "react";
import { isAllowedGlobalShortcut, shortcutFromEvent } from "../lib/hotkeys";
import type { HotkeyConfig } from "../types";

const unsafeShortcutError = "单键快捷键仅支持 F1-F24；其他按键请搭配修饰键。";

const fields: Array<{ id: keyof HotkeyConfig; label: string }> = [
  { id: "record", label: "录制" },
  { id: "playback", label: "播放" },
  { id: "stop", label: "停止" }
];

interface HotkeyPanelProps {
  hotkeys: HotkeyConfig;
  disabled: boolean;
  onSave: (config: HotkeyConfig) => void;
}

export function HotkeyPanel({ hotkeys, disabled, onSave }: HotkeyPanelProps) {
  const [draft, setDraft] = useState(hotkeys);
  const [capturingField, setCapturingField] = useState<keyof HotkeyConfig | null>(null);
  const [captureError, setCaptureError] = useState("");

  useEffect(() => {
    setDraft(hotkeys);
  }, [hotkeys]);

  function captureShortcut(field: keyof HotkeyConfig, event: KeyboardEvent<HTMLButtonElement>) {
    if (capturingField !== field) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();

    if (
      event.key === "Escape" &&
      !event.ctrlKey &&
      !event.altKey &&
      !event.shiftKey &&
      !event.metaKey
    ) {
      cancelCapture();
      return;
    }

    const shortcut = shortcutFromEvent(event);
    if (!shortcut) {
      return;
    }

    if (!isAllowedGlobalShortcut(shortcut)) {
      setCaptureError(unsafeShortcutError);
      return;
    }

    setDraft((current) => ({ ...current, [field]: shortcut }));
    setCapturingField(null);
    setCaptureError("");
  }

  function startCapture(field: keyof HotkeyConfig) {
    setCapturingField(field);
    setCaptureError("");
  }

  function cancelCapture() {
    setCapturingField(null);
    setCaptureError("");
  }

  return (
    <section className="panel hotkey-panel" aria-labelledby="hotkeys-title">
      <h2 id="hotkeys-title">快捷键</h2>
      <form
        className="hotkey-form"
        onSubmit={(event) => {
          event.preventDefault();
          if (Object.values(draft).some((shortcut) => !isAllowedGlobalShortcut(shortcut))) {
            setCaptureError(unsafeShortcutError);
            return;
          }
          onSave(draft);
        }}
      >
        {fields.map((field) => (
          <label className="hotkey-field" key={field.id}>
            <span>{field.label}</span>
            <button
              className="hotkey-capture-button"
              type="button"
              aria-label={`${field.label}快捷键`}
              aria-pressed={capturingField === field.id}
              onClick={() => startCapture(field.id)}
              onKeyDown={(event) => captureShortcut(field.id, event)}
              disabled={disabled}
            >
              {capturingField === field.id ? "请按快捷键" : draft[field.id] || "未设置"}
            </button>
            <kbd>{draft[field.id] || "未设置"}</kbd>
          </label>
        ))}
        {capturingField ? (
          <button
            className="capture-cancel-button"
            type="button"
            aria-label="取消快捷键捕获"
            onClick={cancelCapture}
          >
            取消捕获
          </button>
        ) : null}
        {captureError ? (
          <p className="alert hotkey-error" role="alert">
            {captureError}
          </p>
        ) : null}
        <button className="action-button compact-button" type="submit" disabled={disabled}>
          <Save size={15} aria-hidden="true" />
          <span className="button-label">保存快捷键</span>
        </button>
      </form>
    </section>
  );
}
