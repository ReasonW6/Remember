import { useEffect, useMemo, useRef, useState } from "react";
import { AdministratorControl } from "./components/AdministratorControl";
import { CompactControls } from "./components/CompactControls";
import { Controls } from "./components/Controls";
import { PlaybackSettings } from "./components/PlaybackSettings";
import { RecordingList } from "./components/RecordingList";
import { StatusPanel } from "./components/StatusPanel";
import { WindowTitlebar } from "./components/WindowTitlebar";
import { shortcutFromEvent } from "./lib/hotkeys";
import * as rememberApi from "./lib/rememberApi";
import { playFeedbackTone } from "./lib/sounds";
import { displayErrorMessage, displayMessage, displayMode } from "./localization";
import type {
  AdvancedSettingsConfig,
  AppMode,
  HotkeyConfig,
  RecordingFile,
  UiState
} from "./types";

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

const defaultAdvancedSettings: AdvancedSettingsConfig = {
  feedback_volume_percent: 50,
  feedback_muted: false,
  show_activity_indicator: true
};

const maxLoopCount = 0xffffffff;
const loopCountError = `循环次数必须是 1 到 ${maxLoopCount} 之间的整数。`;
const speedError = "速度必须是大于 0 的有效数字。";
const lastRecordingPathKey = "remember:last-recording-path";
const compactWindowSize = { width: 360, height: 134 };
const expandedWindowSize = { width: 420, height: 520 };

interface PlaybackSettingsValue {
  loopCount: number | null;
  speedMultiplier: number;
}

const defaultPlaybackSettings: PlaybackSettingsValue = {
  loopCount: 1,
  speedMultiplier: 1
};

export function App() {
  const [compactMode, setCompactMode] = useState(true);
  const [windowResizePending, setWindowResizePending] = useState(false);
  const [state, setState] = useState<UiState>(idleState);
  const [loopCount, setLoopCount] = useState<number | null>(1);
  const [speedMultiplier, setSpeedMultiplier] = useState(1);
  const [actionError, setActionError] = useState("");
  const [initializationErrors, setInitializationErrors] = useState<string[]>([]);
  const [recordings, setRecordings] = useState<RecordingFile[]>([]);
  const [selectedRecordingPath, setSelectedRecordingPath] = useState<string | null>(null);
  const [hotkeys, setHotkeys] = useState(defaultHotkeys);
  const [isElevated, setIsElevated] = useState(false);
  const [pendingCommand, setPendingCommand] = useState(false);
  const [playbackSettingsReady, setPlaybackSettingsReady] = useState(false);
  const [playbackSettingsPending, setPlaybackSettingsPending] = useState(true);
  const [playbackSettingsError, setPlaybackSettingsError] = useState("");
  const [appliedPlaybackSettings, setAppliedPlaybackSettings] =
    useState<PlaybackSettingsValue>(defaultPlaybackSettings);
  const pendingCommandRef = useRef(false);
  const playbackSettingsSyncRef = useRef(0);
  const playbackSettingsQueueRef = useRef<Promise<void>>(Promise.resolve());
  const latestRevisionRef = useRef(idleState.revision);
  const previousModeRef = useRef(idleState.mode);
  const advancedSettingsRef = useRef(defaultAdvancedSettings);
  const recordingsChangeVersionRef = useRef(0);
  const recordingsRefreshRef = useRef<Promise<void> | null>(null);
  const hotkeysChangeVersionRef = useRef(0);
  const advancedSettingsChangeVersionRef = useRef(0);
  const hasRecording = state.step_count > 0;
  const isBusy = state.mode === "recording" || state.mode === "playing";
  const validationError = useMemo(() => {
    if (
      loopCount !== null &&
      (!Number.isInteger(loopCount) || loopCount < 1 || loopCount > maxLoopCount)
    ) {
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
    let unsubscribeHotkeys: (() => void) | undefined;
    let unsubscribeAdvancedSettings: (() => void) | undefined;
    let deferredInitializationFrame: number | undefined;

    async function initializeState() {
      try {
        const nextUnsubscribe = await rememberApi.subscribeToState((nextState) => {
          if (!disposed) {
            applyUiState(nextState);
          }
        });
        if (disposed) {
          nextUnsubscribe();
          return;
        }
        unsubscribeState = nextUnsubscribe;
      } catch (subscribeError) {
        if (!disposed) {
          addInitializationError(subscribeError);
        }
      }

      if (disposed) {
        return;
      }
      try {
        applyUiState(await rememberApi.getState());
      } catch (loadError) {
        if (!disposed) {
          addInitializationError(loadError);
        }
      }
    }

    async function initializeRecordings() {
      try {
        const nextUnsubscribe = await rememberApi.subscribeToRecordingsChanged(() => {
          if (disposed) {
            return;
          }
          void refreshRecordings().catch((refreshError: unknown) => {
            if (!disposed) {
              setActionError(displayErrorMessage(refreshError));
            }
          });
        });
        if (disposed) {
          nextUnsubscribe();
          return;
        }
        unsubscribeRecordings = nextUnsubscribe;
      } catch (subscribeError) {
        if (!disposed) {
          addInitializationError(subscribeError);
        }
      }

      if (disposed) {
        return;
      }
      const changeVersion = recordingsChangeVersionRef.current;
      try {
        const nextRecordings = await rememberApi.listRecordings();
        if (!disposed && changeVersion === recordingsChangeVersionRef.current) {
          setRecordings(nextRecordings);
        }
      } catch (loadError) {
        if (!disposed) {
          addInitializationError(loadError);
        }
      }
    }

    async function initializeHotkeys() {
      try {
        const nextUnsubscribe = await rememberApi.subscribeToHotkeysChanged((nextHotkeys) => {
          if (!disposed) {
            hotkeysChangeVersionRef.current += 1;
            setHotkeys(nextHotkeys);
          }
        });
        if (disposed) {
          nextUnsubscribe();
          return;
        }
        unsubscribeHotkeys = nextUnsubscribe;
      } catch (subscribeError) {
        if (!disposed) {
          addInitializationError(subscribeError);
        }
      }

      if (disposed) {
        return;
      }
      const changeVersion = hotkeysChangeVersionRef.current;
      try {
        const nextHotkeys = await rememberApi.getHotkeys();
        if (!disposed && changeVersion === hotkeysChangeVersionRef.current) {
          setHotkeys(nextHotkeys);
        }
      } catch (loadError) {
        if (!disposed) {
          addInitializationError(loadError);
        }
      }
    }

    async function initializeAdvancedSettings() {
      try {
        const nextUnsubscribe = await rememberApi.subscribeToAdvancedSettingsChanged(
          (settings) => {
            if (!disposed) {
              advancedSettingsChangeVersionRef.current += 1;
              advancedSettingsRef.current = settings;
            }
          }
        );
        if (disposed) {
          nextUnsubscribe();
          return;
        }
        unsubscribeAdvancedSettings = nextUnsubscribe;
      } catch (subscribeError) {
        if (!disposed) {
          addInitializationError(subscribeError);
        }
      }

      if (disposed) {
        return;
      }
      const changeVersion = advancedSettingsChangeVersionRef.current;
      try {
        const settings = await rememberApi.getAdvancedSettings();
        if (!disposed && changeVersion === advancedSettingsChangeVersionRef.current) {
          advancedSettingsRef.current = settings;
        }
      } catch (loadError) {
        if (!disposed) {
          addInitializationError(loadError);
        }
      }
    }

    async function restoreLastRecording() {
      const path = readLastRecordingPath();
      if (!path) {
        return;
      }
      try {
        const loadedState = await rememberApi.loadRecording(path);
        if (!disposed && applyUiState(loadedState)) {
          setSelectedRecordingPath(path);
        }
      } catch {
        clearLastRecordingPath();
      }
    }

    void initializeState();
    deferredInitializationFrame = window.requestAnimationFrame(() => {
      deferredInitializationFrame = undefined;
      if (disposed) {
        return;
      }
      void initializeRecordings();
      void initializeHotkeys();
      void initializeAdvancedSettings();
      void restoreLastRecording();
      void rememberApi
        .getPrivilegeState()
        .then((privilegeState) => {
          if (!disposed) {
            setIsElevated(privilegeState.is_elevated);
          }
        })
        .catch((loadError: unknown) => {
          if (!disposed) {
            addInitializationError(loadError);
          }
        });
    });

    return () => {
      disposed = true;
      if (deferredInitializationFrame !== undefined) {
        window.cancelAnimationFrame(deferredInitializationFrame);
      }
      unsubscribeState?.();
      unsubscribeRecordings?.();
      unsubscribeHotkeys?.();
      unsubscribeAdvancedSettings?.();
    };
  }, []);

  useEffect(() => {
    const syncRevision = ++playbackSettingsSyncRef.current;
    if (validationError) {
      setPlaybackSettingsReady(false);
      setPlaybackSettingsPending(false);
      setPlaybackSettingsError("");
      return;
    }

    const requestedSettings = { loopCount, speedMultiplier };
    setPlaybackSettingsReady(false);
    setPlaybackSettingsPending(true);
    setPlaybackSettingsError("");
    const synchronization = playbackSettingsQueueRef.current.then(() =>
      rememberApi.setPlaybackSettings(
        requestedSettings.loopCount,
        requestedSettings.speedMultiplier
      )
    );
    playbackSettingsQueueRef.current = synchronization.catch(() => undefined);

    void synchronization
      .then(() => {
        setAppliedPlaybackSettings(requestedSettings);
        if (syncRevision === playbackSettingsSyncRef.current) {
          setPlaybackSettingsPending(false);
          setPlaybackSettingsReady(true);
        }
      })
      .catch((settingsError: unknown) => {
        if (syncRevision === playbackSettingsSyncRef.current) {
          setPlaybackSettingsPending(false);
          setPlaybackSettingsError(displayErrorMessage(settingsError));
        }
      });
  }, [loopCount, speedMultiplier, validationError]);

  useEffect(() => {
    if (state.mode === "recording") {
      setSelectedRecordingPath(null);
    }
  }, [state.mode]);

  useEffect(() => {
    if (state.mode !== "idle" || selectedRecordingPath || !state.recording_name) {
      return;
    }
    const recording = recordings.find(
      (candidate) => !candidate.load_error && candidate.name === state.recording_name
    );
    if (recording) {
      setSelectedRecordingPath(recording.path);
      writeLastRecordingPath(recording.path);
    }
  }, [recordings, selectedRecordingPath, state.mode, state.recording_name]);

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

    const settings = advancedSettingsRef.current;
    if (nextMode === "recording") {
      playFeedbackTone(
        "recording_start",
        settings.feedback_volume_percent,
        settings.feedback_muted
      );
    } else if (previousMode === "recording") {
      playFeedbackTone(
        "recording_stop",
        settings.feedback_volume_percent,
        settings.feedback_muted
      );
    } else if (nextMode === "playing") {
      playFeedbackTone(
        "playback_start",
        settings.feedback_volume_percent,
        settings.feedback_muted
      );
    } else if (previousMode === "playing") {
      playFeedbackTone(
        "playback_stop",
        settings.feedback_volume_percent,
        settings.feedback_muted
      );
    }

    previousModeRef.current = nextMode;
  }

  function applyUiState(nextState: UiState) {
    if (nextState.revision < latestRevisionRef.current) {
      return false;
    }

    const previousMode = previousModeRef.current;
    latestRevisionRef.current = nextState.revision;
    announceModeTransition(nextState.mode);
    setState(nextState);
    if (previousMode === "recording" && nextState.mode === "idle") {
      void refreshRecordings().catch((refreshError: unknown) => {
        setActionError(displayErrorMessage(refreshError));
      });
    }
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

  function refreshRecordings() {
    const requestedVersion = ++recordingsChangeVersionRef.current;
    if (recordingsRefreshRef.current) {
      return recordingsRefreshRef.current;
    }

    const refresh = (async () => {
      let refreshVersion = requestedVersion;
      while (true) {
        try {
          const nextRecordings = await rememberApi.listRecordings();
          if (refreshVersion === recordingsChangeVersionRef.current) {
            setRecordings(nextRecordings);
            return;
          }
        } catch (refreshError) {
          if (refreshVersion === recordingsChangeVersionRef.current) {
            throw refreshError;
          }
        }
        refreshVersion = recordingsChangeVersionRef.current;
      }
    })();
    recordingsRefreshRef.current = refresh;
    void refresh.then(
      () => {
        if (recordingsRefreshRef.current === refresh) {
          recordingsRefreshRef.current = null;
        }
      },
      () => {
        if (recordingsRefreshRef.current === refresh) {
          recordingsRefreshRef.current = null;
        }
      }
    );
    return refresh;
  }

  function handleRecord() {
    if (state.mode === "recording") {
      void applyState(rememberApi.stopRecording);
      return;
    }

    void applyState(rememberApi.startRecording);
  }

  function handlePlay() {
    if (validationError || !playbackSettingsReady) {
      return;
    }
    void applyState(() =>
      rememberApi.startPlayback(
        appliedPlaybackSettings.loopCount,
        appliedPlaybackSettings.speedMultiplier
      )
    );
  }

  function handleStop() {
    if (state.mode === "recording") {
      void applyState(rememberApi.stopRecording);
      return;
    }

    void applyState(rememberApi.stopPlayback);
  }

  function handleSave() {
    void applyCommand(rememberApi.saveCurrentRecording);
  }

  function handleOpen() {
    void applyCommand(async () => {
      const opened = await rememberApi.openRecording();
      if (opened && applyUiState(opened.state)) {
        setSelectedRecordingPath(opened.path);
        writeLastRecordingPath(opened.path);
      }
    });
  }

  function handleSelectRecording(path: string) {
    void applyCommand(async () => {
      const loadedState = await rememberApi.loadRecording(path);
      if (applyUiState(loadedState)) {
        setSelectedRecordingPath(path);
        writeLastRecordingPath(path);
      }
    });
  }

  function handleToggleWindowSize() {
    if (windowResizePending) {
      return;
    }
    const nextCompactMode = !compactMode;
    const size = nextCompactMode ? compactWindowSize : expandedWindowSize;
    setCompactMode(nextCompactMode);
    setWindowResizePending(true);
    void import("@tauri-apps/api/window")
      .then(({ getCurrentWindow, LogicalSize }) =>
        getCurrentWindow().setSize(new LogicalSize(size.width, size.height))
      )
      .then(() => {
        setActionError("");
      })
      .catch((resizeError: unknown) => {
        setCompactMode(!nextCompactMode);
        setActionError(displayErrorMessage(resizeError));
      })
      .finally(() => {
        setWindowResizePending(false);
      });
  }

  function handleRefreshRecordings() {
    void applyCommand(refreshRecordings);
  }

  function handleAdvancedSettings() {
    void applyCommand(rememberApi.showAdvancedSettings);
  }

  function handleRestartAsAdministrator() {
    void applyCommand(rememberApi.restartAsAdministrator);
  }

  function handleDeleteRecording(recording: RecordingFile, force: boolean) {
    void applyCommand(async () => {
      if (!force && !(await rememberApi.confirmDeleteRecording(recording.name))) {
        return;
      }

      await rememberApi.deleteRecording(recording.path);
      if (selectedRecordingPath === recording.path) {
        setSelectedRecordingPath(null);
        clearLastRecordingPath();
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
          writeLastRecordingPath(renamedPath);
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
        } else if (
          state.mode === "idle" &&
          hasRecording &&
          !validationError &&
          playbackSettingsReady
        ) {
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
  }, [
    hasRecording,
    hotkeys,
    state.mode,
    validationError,
    playbackSettingsReady,
    appliedPlaybackSettings
  ]);

  const displayedError = [
    ...initializationErrors,
    actionError,
    playbackSettingsError
  ]
    .filter(Boolean)
    .join(" ");
  const displayedStateMessage = state.message_is_error
    ? displayErrorMessage(state.message)
    : displayMessage(state.message);

  return (
    <main className={compactMode ? "app-shell compact-shell" : "app-shell"}>
      <WindowTitlebar
        compact={compactMode}
        resizePending={windowResizePending}
        onToggleSize={handleToggleWindowSize}
      />
      {compactMode ? (
        <CompactControls
          state={state}
          recordings={recordings}
          selectedPath={selectedRecordingPath}
          selectedName={state.recording_name}
          hasRecording={hasRecording}
          playbackValid={!validationError && playbackSettingsReady}
          pendingCommand={pendingCommand}
          isElevated={isElevated}
          message={displayedStateMessage}
          error={displayedError}
          onSelect={handleSelectRecording}
          onRecord={handleRecord}
          onPlay={handlePlay}
          onStop={handleStop}
          onRestartAsAdministrator={handleRestartAsAdministrator}
        />
      ) : (
        <div className="app-content">
        <header className="app-header">
          <div className="brand-block">
            <img className="app-icon" src="/remember-icon.svg" alt="Remember 图标" />
            <div>
              <h1>Remember</h1>
              <p>模式：{displayMode(state.mode)}</p>
            </div>
          </div>
          <p
            className="mode-summary"
            role="status"
            aria-live="polite"
            aria-atomic="true"
          >
            {displayedStateMessage}
          </p>
        </header>
        <div className="content-grid">
          <div className="main-stack">
            <Controls
              state={state}
              hasRecording={hasRecording}
              playbackValid={!validationError && playbackSettingsReady}
              pendingCommand={pendingCommand}
              onRecord={handleRecord}
              onPlay={handlePlay}
              onStop={handleStop}
              onSave={handleSave}
              onOpen={handleOpen}
              onAdvancedSettings={handleAdvancedSettings}
            />
            <AdministratorControl
              isElevated={isElevated}
              disabled={pendingCommand || isBusy}
              onRestart={handleRestartAsAdministrator}
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
              appliedLoopCount={appliedPlaybackSettings.loopCount}
              appliedSpeedMultiplier={appliedPlaybackSettings.speedMultiplier}
              syncPending={playbackSettingsPending}
              syncReady={playbackSettingsReady}
              playbackHotkey={hotkeys.playback}
              onLoopCountChange={setLoopCount}
              onSpeedMultiplierChange={setSpeedMultiplier}
            />
            <StatusPanel state={state} error={displayedError} />
          </div>
        </div>
        </div>
      )}
    </main>
  );
}

function readLastRecordingPath() {
  try {
    return window.localStorage.getItem(lastRecordingPathKey);
  } catch {
    return null;
  }
}

function writeLastRecordingPath(path: string) {
  try {
    window.localStorage.setItem(lastRecordingPathKey, path);
  } catch {
    // The selector still works for the current session when storage is unavailable.
  }
}

function clearLastRecordingPath() {
  try {
    window.localStorage.removeItem(lastRecordingPathKey);
  } catch {
    // Ignore unavailable storage; there is no persistent state to clear.
  }
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
