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
import type { AppMode, HotkeyConfig, RecordingFile, UiState } from "./types";

const idleState: UiState = {
  mode: "idle",
  recording_name: null,
  step_count: 0,
  duration_ms: 0,
  message: "Idle",
  revision: 0,
  message_is_error: false
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
  const [loopCount, setLoopCount] = useState<number | null>(1);
  const [speedMultiplier, setSpeedMultiplier] = useState(1);
  const [actionError, setActionError] = useState("");
  const [initializationErrors, setInitializationErrors] = useState<string[]>([]);
  const [recordings, setRecordings] = useState<RecordingFile[]>([]);
  const [selectedRecordingPath, setSelectedRecordingPath] = useState<string | null>(null);
  const [hotkeys, setHotkeys] = useState(defaultHotkeys);
  const [pendingCommand, setPendingCommand] = useState(false);
  const pendingCommandRef = useRef(false);
  const latestRevisionRef = useRef(idleState.revision);
  const previousModeRef = useRef(idleState.mode);
  const hasRecording = state.step_count > 0;
  const isBusy = state.mode === "recording" || state.mode === "playing";
  const validationError = useMemo(() => {
    if (loopCount !== null && (!Number.isInteger(loopCount) || loopCount < 1)) {
      return loopCountError;
    }
    if (!Number.isFinite(speedMultiplier) || speedMultiplier <= 0) {
      return speedError;
    }
    return "";
  }, [loopCount, speedMultiplier]);

  useEffect(() => {
    let disposed = false;
    let unsubscribeState: (() => void) | undefined;
    let unsubscribeRecordings: (() => void) | undefined;

    rememberApi
      .getState()
      .then((nextState) => {
        if (!disposed) {
          applyUiState(nextState);
        }
      })
      .catch((loadError: unknown) => {
        if (!disposed) {
          addInitializationError(loadError);
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
          addInitializationError(loadError);
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
          addInitializationError(loadError);
        }
      });

    rememberApi
      .subscribeToState((nextState) => {
        if (!disposed) {
          applyUiState(nextState);
        }
      })
      .then((nextUnsubscribe) => {
        unsubscribeState = nextUnsubscribe;
        if (disposed) {
          unsubscribeState();
        }
      })
      .catch((subscribeError: unknown) => {
        if (!disposed) {
          addInitializationError(subscribeError);
        }
      });

    rememberApi
      .subscribeToRecordingsChanged(() => {
        if (disposed) {
          return;
        }
        void refreshRecordings().catch((refreshError: unknown) => {
          if (!disposed) {
            setActionError(displayErrorMessage(refreshError));
          }
        });
      })
      .then((nextUnsubscribe) => {
        unsubscribeRecordings = nextUnsubscribe;
        if (disposed) {
          unsubscribeRecordings();
        }
      })
      .catch((subscribeError: unknown) => {
        if (!disposed) {
          addInitializationError(subscribeError);
        }
      });

    return () => {
      disposed = true;
      unsubscribeState?.();
      unsubscribeRecordings?.();
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
          setActionError(displayErrorMessage(settingsError));
        }
      });

    return () => {
      disposed = true;
    };
  }, [loopCount, speedMultiplier, validationError]);

  useEffect(() => {
    if (state.mode === "recording") {
      setSelectedRecordingPath(null);
    }
  }, [state.mode]);

  function addInitializationError(error: unknown) {
    const message = displayErrorMessage(error);
    setInitializationErrors((current) =>
      current.includes(message) ? current : [...current, message]
    );
  }

  function announceModeTransition(nextMode: AppMode) {
    const previousMode = previousModeRef.current;
    if (previousMode === nextMode) {
      return;
    }

    if (nextMode === "recording") {
      playFeedbackTone("recording_start");
    } else if (previousMode === "recording") {
      playFeedbackTone("recording_stop");
    } else if (nextMode === "playing") {
      playFeedbackTone("playback_start");
    } else if (previousMode === "playing") {
      playFeedbackTone("playback_stop");
    }

    previousModeRef.current = nextMode;
  }

  function applyUiState(nextState: UiState) {
    if (nextState.revision < latestRevisionRef.current) {
      return false;
    }

    latestRevisionRef.current = nextState.revision;
    announceModeTransition(nextState.mode);
    setState(nextState);
    return true;
  }

  async function applyCommand(action: () => Promise<void>) {
    if (pendingCommandRef.current) {
      return;
    }

    pendingCommandRef.current = true;
    setPendingCommand(true);

    try {
      setActionError("");
      await action();
    } catch (actionError) {
      setActionError(displayErrorMessage(actionError));
    } finally {
      pendingCommandRef.current = false;
      setPendingCommand(false);
    }
  }

  function applyState(action: () => Promise<UiState>) {
    return applyCommand(async () => {
      applyUiState(await action());
    });
  }

  async function refreshRecordings() {
    setRecordings(await rememberApi.listRecordings());
  }

  function handleRecord() {
    if (state.mode === "recording") {
      void applyCommand(async () => {
        applyUiState(await rememberApi.stopRecording());
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
        applyUiState(await rememberApi.stopRecording());
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
      if (loadedState && applyUiState(loadedState)) {
        setSelectedRecordingPath(null);
      }
    });
  }

  function handleSelectRecording(path: string) {
    void applyCommand(async () => {
      const loadedState = await rememberApi.loadRecording(path);
      if (applyUiState(loadedState)) {
        setSelectedRecordingPath(path);
      }
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

  function handleDeleteRecording(recording: RecordingFile, force: boolean) {
    void applyCommand(async () => {
      if (!force && !(await rememberApi.confirmDeleteRecording(recording.name))) {
        return;
      }

      await rememberApi.deleteRecording(recording.path);
      if (selectedRecordingPath === recording.path) {
        setSelectedRecordingPath(null);
      }
      await refreshRecordings();
    });
  }

  function handleRenameRecording(recording: RecordingFile, newName: string) {
    void applyCommand(async () => {
      const renamedPath = await rememberApi.renameRecording(recording.path, newName);
      if (selectedRecordingPath === recording.path) {
        const loadedState = await rememberApi.loadRecording(renamedPath);
        if (applyUiState(loadedState)) {
          setSelectedRecordingPath(renamedPath);
        }
      }
      await refreshRecordings();
    });
  }

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.repeat || event.isComposing || shouldIgnoreAppHotkey(event)) {
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

      if (shortcut === hotkeys.playback) {
        if (state.mode === "playing") {
          event.preventDefault();
          handleStop();
        } else if (state.mode === "idle" && hasRecording && !validationError) {
          event.preventDefault();
          handlePlay();
        }
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

  const displayedError = [...initializationErrors, actionError].filter(Boolean).join(" ");
  const displayedStateMessage = state.message_is_error
    ? displayErrorMessage(state.message)
    : displayMessage(state.message);

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
          <p className="mode-summary">{displayedStateMessage}</p>
        </header>
        <div className="content-grid">
          <div className="main-stack">
            <Controls
              state={state}
              hasRecording={hasRecording}
              playbackValid={!validationError}
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
              onRename={handleRenameRecording}
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

function shouldIgnoreAppHotkey(event: KeyboardEvent) {
  const target = event.target;
  if (!(target instanceof Element)) {
    return false;
  }

  if (target.closest(".hotkey-capture-button")) {
    return true;
  }

  const editableTarget = target.closest("input, textarea, select, [contenteditable='true']");
  return editableTarget !== null && !/^F([1-9]|1[0-9]|2[0-4])$/.test(event.key);
}
