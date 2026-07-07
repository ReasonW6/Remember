import { Save } from "lucide-react";
import { type KeyboardEvent, useEffect, useState } from "react";
import { shortcutFromEvent } from "../lib/hotkeys";
import type { HotkeyConfig } from "../types";

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

  useEffect(() => {
    setDraft(hotkeys);
  }, [hotkeys]);

  function captureShortcut(field: keyof HotkeyConfig, event: KeyboardEvent<HTMLButtonElement>) {
    if (capturingField !== field) {
      return;
    }

    const shortcut = shortcutFromEvent(event);
    event.preventDefault();
    event.stopPropagation();

    if (!shortcut) {
      return;
    }

    setDraft((current) => ({ ...current, [field]: shortcut }));
    setCapturingField(null);
  }

  return (
    <section className="panel hotkey-panel" aria-labelledby="hotkeys-title">
      <h2 id="hotkeys-title">快捷键</h2>
      <form
        className="hotkey-form"
        onSubmit={(event) => {
          event.preventDefault();
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
              onClick={() => setCapturingField(field.id)}
              onKeyDown={(event) => captureShortcut(field.id, event)}
              onBlur={() => setCapturingField(null)}
              disabled={disabled}
            >
              {capturingField === field.id ? "请按快捷键" : draft[field.id] || "未设置"}
            </button>
            <kbd>{draft[field.id] || "未设置"}</kbd>
          </label>
        ))}
        <button className="action-button compact-button" type="submit" disabled={disabled}>
          <Save size={15} aria-hidden="true" />
          <span className="button-label">保存快捷键</span>
        </button>
      </form>
    </section>
  );
}
