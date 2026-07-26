import { Save, Volume2, VolumeX } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { HotkeyPanel } from "./components/HotkeyPanel";
import { WindowTitlebar } from "./components/WindowTitlebar";
import * as rememberApi from "./lib/rememberApi";
import { playFeedbackTone } from "./lib/sounds";
import { displayErrorMessage } from "./localization";
import type { AdvancedSettingsConfig, HotkeyConfig } from "./types";

const defaultSettings: AdvancedSettingsConfig = {
  feedback_volume_percent: 50,
  feedback_muted: false,
  show_activity_indicator: true
};

const defaultHotkeys: HotkeyConfig = {
  record: "F8",
  playback: "F12",
  stop: "F8"
};

export function AdvancedSettings() {
  const [settings, setSettings] = useState(defaultSettings);
  const [hotkeys, setHotkeys] = useState(defaultHotkeys);
  const [pending, setPending] = useState(true);
  const [error, setError] = useState("");
  const [showSavedNotice, setShowSavedNotice] = useState(false);
  const volumePreviewTimerRef = useRef<number | undefined>();
  const savedNoticeTimerRef = useRef<number | undefined>();

  useEffect(() => {
    let disposed = false;
    void Promise.all([rememberApi.getAdvancedSettings(), rememberApi.getHotkeys()])
      .then(([loadedSettings, loadedHotkeys]) => {
        if (!disposed) {
          setSettings(loadedSettings);
          setHotkeys(loadedHotkeys);
        }
      })
      .catch((loadError: unknown) => {
        if (!disposed) {
          setError(displayErrorMessage(loadError));
        }
      })
      .finally(() => {
        if (!disposed) {
          setPending(false);
        }
      });

    return () => {
      disposed = true;
      window.clearTimeout(volumePreviewTimerRef.current);
      window.clearTimeout(savedNoticeTimerRef.current);
    };
  }, []);

  function previewVolume(volumePercent: number, muted: boolean) {
    window.clearTimeout(volumePreviewTimerRef.current);
    if (muted || volumePercent === 0) {
      return;
    }
    volumePreviewTimerRef.current = window.setTimeout(() => {
      playFeedbackTone("recording_start", volumePercent, false);
    }, 250);
  }

  function toggleMute() {
    const nextMuted = !settings.feedback_muted;
    setSettings((current) => ({ ...current, feedback_muted: nextMuted }));
    previewVolume(settings.feedback_volume_percent, nextMuted);
  }

  function showSavedToast() {
    window.clearTimeout(savedNoticeTimerRef.current);
    setShowSavedNotice(true);
    savedNoticeTimerRef.current = window.setTimeout(() => {
      setShowSavedNotice(false);
    }, 1800);
  }

  async function saveAllSettings() {
    if (pending) {
      return;
    }
    setPending(true);
    setError("");
    setShowSavedNotice(false);
    try {
      const savedHotkeys = await rememberApi.setHotkeys(hotkeys);
      setSettings(await rememberApi.setAdvancedSettings(settings));
      setHotkeys(savedHotkeys);
      showSavedToast();
    } catch (saveError) {
      setError(displayErrorMessage(saveError));
    } finally {
      setPending(false);
    }
  }

  return (
    <main className="app-shell advanced-settings-shell">
      <WindowTitlebar subtitle="高级设置" showMinimize={false} />
      <div className="app-content advanced-settings-content">
        <header className="advanced-settings-header">
          <div>
            <h1>高级设置</h1>
            <p>调整操作反馈、全局快捷键与屏幕提示。</p>
          </div>
          <button
            className="action-button advanced-settings-save-button"
            type="button"
            disabled={pending}
            onClick={() => void saveAllSettings()}
          >
            <Save size={15} aria-hidden="true" />
            <span>保存</span>
          </button>
        </header>

        {showSavedNotice ? (
          <div className="settings-toast" role="status">
            已保存
          </div>
        ) : null}
        {error ? (
          <p className="alert" role="alert">
            {error}
          </p>
        ) : null}

        <section className="panel advanced-setting-panel" aria-labelledby="sound-settings-title">
          <div className="section-heading">
            <h2 id="sound-settings-title">提示音音量</h2>
            <output htmlFor="feedback-volume">{settings.feedback_volume_percent}%</output>
          </div>
          <div className="volume-controls">
            <input
              id="feedback-volume"
              type="range"
              min="0"
              max="100"
              step="1"
              aria-label="提示音音量"
              value={settings.feedback_volume_percent}
              disabled={pending || settings.feedback_muted}
              onChange={(event) => {
                const volumePercent = Number(event.target.value);
                setSettings((current) => ({
                  ...current,
                  feedback_volume_percent: volumePercent
                }));
                previewVolume(volumePercent, settings.feedback_muted);
              }}
            />
            <button
              className="action-button mute-button"
              type="button"
              disabled={pending}
              aria-pressed={settings.feedback_muted}
              onClick={toggleMute}
            >
              {settings.feedback_muted ? (
                <Volume2 size={16} aria-hidden="true" />
              ) : (
                <VolumeX size={16} aria-hidden="true" />
              )}
              <span>{settings.feedback_muted ? "恢复声音" : "静音"}</span>
            </button>
          </div>
        </section>

        <HotkeyPanel hotkeys={hotkeys} disabled={pending} onChange={setHotkeys} />

        <section
          className="panel advanced-setting-panel"
          aria-labelledby="indicator-settings-title"
        >
          <h2 id="indicator-settings-title">屏幕提示</h2>
          <label className="indicator-setting">
            <input
              type="checkbox"
              checked={settings.show_activity_indicator}
              disabled={pending}
              onChange={(event) =>
                setSettings((current) => ({
                  ...current,
                  show_activity_indicator: event.target.checked
                }))
              }
            />
            <span>显示左上角操作提示</span>
          </label>
        </section>
      </div>
    </main>
  );
}
