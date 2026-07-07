import { useEffect, useMemo, useRef, useState } from "react";
import { Controls } from "./components/Controls";
import { HotkeyPanel } from "./components/HotkeyPanel";
import { PlaybackSettings } from "./components/PlaybackSettings";
import { RecordingList } from "./components/RecordingList";
import { StatusPanel } from "./components/StatusPanel";
import { WindowTitlebar } from "./components/WindowTitlebar";
import { shortcutFromEvent } from "./lib/hotkeys";
import * as rememberApi from "./lib/rememberApi";
import { playFeedbackTone } from "./lib/sounds";
import { displayErrorMessage, displayMessage, displayMode } from "./localization";
import "./styles.css";
import type { HotkeyConfig, RecordingFile, UiState } from "./types";

const idleState: UiState = {
  mode: "idle",
  recording_name: null,
  step_count: 0,
  duration_ms: 0,
  message: "Idle"
};

const defaultHotkeys: HotkeyConfig = {
  record: "F8",
  playback: "F12",
  stop: "F8"
};

const loopCountError = "循环次数必须是大于等于 1 的整数。";
const speedError = "速度必须是大于 0 的有效数字。";

export function App() {
  const [state, setState] = useState<UiState>(idleState);
  const [loopCount, setLoopCount] = useState(1);
  const [speedMultiplier, setSpeedMultiplier] = useState(1);
  const [error, setError] = useState("");
  const [recordings, setRecordings] = useState<RecordingFile[]>([]);
  const [selectedRecordingPath, setSelectedRecordingPath] = useState<string | null>(null);
  const [hotkeys, setHotkeys] = useState(defaultHotkeys);
  const [pendingCommand, setPendingCommand] = useState(false);
  const pendingCommandRef = useRef(false);
  const previousModeRef = useRef(idleState.mode);
  const hasSoundBaselineRef = useRef(false);
  const hasRecording = state.step_count > 0;
  const isBusy = state.mode === "recording" || state.mode === "playing";
  const validationError = useMemo(() => {
    if (!Number.isSafeInteger(loopCount) || loopCount < 1) {
      return loopCountError;
    }
    if (!Number.isFinite(speedMultiplier) || speedMultiplier <= 0) {
      return speedError;
    }
    return "";
  }, [loopCount, speedMultiplier]);

  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | undefined;

    rememberApi
      .getState()
      .then((nextState) => {
        if (!disposed) {
          setState(nextState);
        }
      })
      .catch((loadError: unknown) => {
        if (!disposed) {
          setError(displayErrorMessage(loadError));
        }
      });

    rememberApi
      .listRecordings()
      .then((nextRecordings) => {
        if (!disposed) {
          setRecordings(nextRecordings);
        }
      })
      .catch((loadError: unknown) => {
        if (!disposed) {
          setError(displayErrorMessage(loadError));
        }
      });

    rememberApi
      .getHotkeys()
      .then((nextHotkeys) => {
        if (!disposed) {
          setHotkeys(nextHotkeys);
        }
      })
      .catch((loadError: unknown) => {
        if (!disposed) {
          setError(displayErrorMessage(loadError));
        }
      });

    rememberApi
      .subscribeToState((nextState) => {
        setState(nextState);
      })
      .then((nextUnsubscribe) => {
        unsubscribe = nextUnsubscribe;
        if (disposed) {
          unsubscribe();
        }
      })
      .catch((subscribeError: unknown) => {
        if (!disposed) {
          setError(displayErrorMessage(subscribeError));
        }
      });

    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, []);

  useEffect(() => {
    if (validationError) {
      return;
    }

    let disposed = false;
    rememberApi
      .setPlaybackSettings(loopCount, speedMultiplier)
      .catch((settingsError: unknown) => {
        if (!disposed) {
          setError(displayErrorMessage(settingsError));
        }
      });

    return () => {
      disposed = true;
    };
  }, [loopCount, speedMultiplier, validationError]);

  useEffect(() => {
    if (!hasSoundBaselineRef.current) {
      hasSoundBaselineRef.current = true;
      previousModeRef.current = state.mode;
      return;
    }

    const previousMode = previousModeRef.current;
    if (previousMode === state.mode) {
      return;
    }

    if (state.mode === "recording") {
      playFeedbackTone("recording_start");
    } else if (previousMode === "recording") {
      playFeedbackTone("recording_stop");
    } else if (state.mode === "playing") {
      playFeedbackTone("playback_start");
    } else if (previousMode === "playing") {
      playFeedbackTone("playback_stop");
    }

    previousModeRef.current = state.mode;
  }, [state.mode]);

  async function applyCommand(action: () => Promise<void>) {
    if (pendingCommandRef.current) {
      return;
    }

    pendingCommandRef.current = true;
    setPendingCommand(true);

    try {
      setError("");
      await action();
    } catch (actionError) {
      setError(displayErrorMessage(actionError));
    } finally {
      pendingCommandRef.current = false;
      setPendingCommand(false);
    }
  }

  function applyState(action: () => Promise<UiState>) {
    return applyCommand(async () => {
      setState(await action());
    });
  }

  async function refreshRecordings() {
    setRecordings(await rememberApi.listRecordings());
  }

  function handleRecord() {
    if (state.mode === "recording") {
      void applyCommand(async () => {
        setState(await rememberApi.stopRecording());
        await refreshRecordings();
      });
      return;
    }

    void applyState(rememberApi.startRecording);
  }

  function handlePlay() {
    if (validationError) {
      return;
    }
    void applyState(() => rememberApi.startPlayback(loopCount, speedMultiplier));
  }

  function handleStop() {
    if (state.mode === "recording") {
      void applyCommand(async () => {
        setState(await rememberApi.stopRecording());
        await refreshRecordings();
      });
      return;
    }

    void applyState(rememberApi.stopPlayback);
  }

  function handleSave() {
    void applyCommand(rememberApi.saveCurrentRecording);
  }

  function handleOpen() {
    void applyCommand(async () => {
      const loadedState = await rememberApi.openRecording();
      if (loadedState) {
        setState(loadedState);
        setSelectedRecordingPath(null);
      }
    });
  }

  function handleSelectRecording(path: string) {
    void applyCommand(async () => {
      const loadedState = await rememberApi.loadRecording(path);
      setState(loadedState);
      setSelectedRecordingPath(path);
    });
  }

  function handleRefreshRecordings() {
    void applyCommand(refreshRecordings);
  }

  function handleSaveHotkeys(config: HotkeyConfig) {
    void applyCommand(async () => {
      setHotkeys(await rememberApi.setHotkeys(config));
    });
  }

  function handleDeleteRecording(path: string) {
    void applyCommand(async () => {
      await rememberApi.deleteRecording(path);
      if (selectedRecordingPath === path) {
        setSelectedRecordingPath(null);
      }
      await refreshRecordings();
    });
  }

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.repeat || isHotkeyCaptureTarget(event.target)) {
        return;
      }

      const shortcut = shortcutFromEvent(event);
      if (!shortcut) {
        return;
      }

      if (shortcut === hotkeys.record) {
        if (state.mode === "idle") {
          event.preventDefault();
          handleRecord();
        } else if (hotkeys.record === hotkeys.stop) {
          event.preventDefault();
          handleStop();
        }
        return;
      }

      if (shortcut === hotkeys.playback && state.mode === "idle" && hasRecording) {
        event.preventDefault();
        handlePlay();
        return;
      }

      if (shortcut === hotkeys.stop && state.mode !== "idle") {
        event.preventDefault();
        handleStop();
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [hasRecording, hotkeys, state.mode, loopCount, speedMultiplier, validationError]);

  const displayedError = error;

  return (
    <main className="app-shell">
      <WindowTitlebar />
      <div className="app-content">
        <header className="app-header">
          <div className="brand-block">
            <img className="app-icon" src="/remember-icon.svg" alt="Remember 图标" />
            <div>
              <h1>Remember</h1>
              <p>模式：{displayMode(state.mode)}</p>
            </div>
          </div>
          <p className="mode-summary">{displayMessage(state.message)}</p>
        </header>
        <div className="content-grid">
          <div className="main-stack">
            <Controls
              state={state}
              hasRecording={hasRecording}
              pendingCommand={pendingCommand}
              onRecord={handleRecord}
              onPlay={handlePlay}
              onStop={handleStop}
              onSave={handleSave}
              onOpen={handleOpen}
            />
            <RecordingList
              recordings={recordings}
              selectedPath={selectedRecordingPath}
              disabled={pendingCommand || isBusy}
              onSelect={handleSelectRecording}
              onDelete={handleDeleteRecording}
              onRefresh={handleRefreshRecordings}
            />
            <PlaybackSettings
              loopCount={loopCount}
              speedMultiplier={speedMultiplier}
              onLoopCountChange={setLoopCount}
              onSpeedMultiplierChange={setSpeedMultiplier}
            />
            <StatusPanel state={state} error={displayedError} />
            <HotkeyPanel
              hotkeys={hotkeys}
              disabled={pendingCommand || isBusy}
              onSave={handleSaveHotkeys}
            />
          </div>
        </div>
      </div>
    </main>
  );
}

function isHotkeyCaptureTarget(target: EventTarget | null) {
  return target instanceof Element && target.closest(".hotkey-capture-button") !== null;
}
