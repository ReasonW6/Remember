import { Save, Volume2, VolumeX } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { HotkeyPanel } from "./components/HotkeyPanel";
import { WindowTitlebar } from "./components/WindowTitlebar";
import * as rememberApi from "./lib/rememberApi";
import { playFeedbackTone } from "./lib/sounds";
import { displayErrorMessage } from "./localization";
import type {
  AdvancedSettingsConfig,
  HotkeyConfig,
  MainWindowPreferences
} from "./types";

const defaultSettings: AdvancedSettingsConfig = {
  feedback_volume_percent: 50,
  feedback_muted: false,
  show_activity_indicator: true,
  window_relative_recording_enabled: false
};

const defaultHotkeys: HotkeyConfig = {
  record: "F8",
  playback: "F12",
  stop: "F8"
};

const defaultMainWindowPreferences: MainWindowPreferences = {
  compact: true,
  position: null
};

const volumeAdjustmentKeys = new Set([
  "ArrowLeft",
  "ArrowRight",
  "ArrowUp",
  "ArrowDown",
  "Home",
  "End",
  "PageUp",
  "PageDown"
]);

async function closeCurrentWindow() {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().close();
}

export function AdvancedSettings() {
  const [settings, setSettings] = useState(defaultSettings);
  const [hotkeys, setHotkeys] = useState(defaultHotkeys);
  const [pending, setPending] = useState(true);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState("");
  const mainWindowPreferencesRef = useRef(defaultMainWindowPreferences);

  useEffect(() => {
    let disposed = false;
    void rememberApi
      .getSettingsBundle()
      .then((bundle) => {
        if (!disposed) {
          setSettings(bundle.advanced);
          setHotkeys(bundle.hotkeys);
          mainWindowPreferencesRef.current = bundle.main_window;
          setLoaded(true);
        }
      })
      .catch((loadError: unknown) => {
        if (!disposed) {
          setError(`设置加载失败，保存已禁用。${displayErrorMessage(loadError)}`);
        }
      })
      .finally(() => {
        if (!disposed) {
          setPending(false);
        }
      });

    return () => {
      disposed = true;
    };
  }, []);

  function previewVolume(volumePercent: number, muted: boolean) {
    if (muted || volumePercent === 0) {
      return;
    }
    playFeedbackTone("recording_start", volumePercent, false);
  }

  function toggleMute() {
    const nextMuted = !settings.feedback_muted;
    setSettings((current) => ({ ...current, feedback_muted: nextMuted }));
    previewVolume(settings.feedback_volume_percent, nextMuted);
  }

  async function saveAllSettings() {
    if (pending || !loaded) {
      return;
    }
    setPending(true);
    setError("");
    try {
      const savedBundle = await rememberApi.setSettingsBundle({
        advanced: settings,
        hotkeys,
        main_window: mainWindowPreferencesRef.current
      });
      setSettings(savedBundle.advanced);
      setHotkeys(savedBundle.hotkeys);
      mainWindowPreferencesRef.current = savedBundle.main_window;
      try {
        await closeCurrentWindow();
      } catch (closeError) {
        setError(`设置已保存，但窗口无法关闭。${displayErrorMessage(closeError)}`);
      }
    } catch (saveError) {
      setError(displayErrorMessage(saveError));
    } finally {
      setPending(false);
    }
  }

  const controlsDisabled = pending || !loaded;

  return (
    <main className="app-shell advanced-settings-shell">
      <WindowTitlebar showMinimize={false} />
      <div className="app-content advanced-settings-content">
        <header className="advanced-settings-header">
          <div>
            <h1>高级设置</h1>
            <p>调整操作反馈、全局快捷键与屏幕提示。</p>
          </div>
          <button
            className="action-button advanced-settings-save-button"
            type="button"
            disabled={controlsDisabled}
            onClick={() => void saveAllSettings()}
          >
            <Save size={15} aria-hidden="true" />
            <span>保存并退出</span>
          </button>
        </header>

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
              disabled={controlsDisabled || settings.feedback_muted}
              onChange={(event) => {
                const volumePercent = Number(event.target.value);
                setSettings((current) => ({
                  ...current,
                  feedback_volume_percent: volumePercent
                }));
              }}
              onPointerUp={(event) =>
                previewVolume(Number(event.currentTarget.value), settings.feedback_muted)
              }
              onKeyUp={(event) => {
                if (volumeAdjustmentKeys.has(event.key)) {
                  previewVolume(
                    Number(event.currentTarget.value),
                    settings.feedback_muted
                  );
                }
              }}
            />
            <button
              className="action-button mute-button"
              type="button"
              disabled={controlsDisabled}
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

        <HotkeyPanel hotkeys={hotkeys} disabled={controlsDisabled} onChange={setHotkeys} />

        <section
          className="panel advanced-setting-panel"
          aria-labelledby="indicator-settings-title"
        >
          <h2 id="indicator-settings-title">屏幕提示</h2>
          <label className="indicator-setting">
            <input
              type="checkbox"
              checked={settings.show_activity_indicator}
              disabled={controlsDisabled}
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
