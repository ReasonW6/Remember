import { useEffect, useMemo, useState } from "react";
import { Controls } from "./components/Controls";
import { HotkeyPanel } from "./components/HotkeyPanel";
import { PlaybackSettings } from "./components/PlaybackSettings";
import { StatusPanel } from "./components/StatusPanel";
import * as rememberApi from "./lib/rememberApi";
import "./styles.css";
import type { UiState } from "./types";

const idleState: UiState = {
  mode: "idle",
  recording_name: null,
  step_count: 0,
  duration_ms: 0,
  message: "Idle"
};

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

export function App() {
  const [state, setState] = useState<UiState>(idleState);
  const [loopCount, setLoopCount] = useState(1);
  const [speedMultiplier, setSpeedMultiplier] = useState(1);
  const [error, setError] = useState("");
  const hasRecording = state.step_count > 0;
  const validationError = useMemo(() => {
    if (loopCount < 1) {
      return "Loop count must be at least 1.";
    }
    if (speedMultiplier <= 0) {
      return "Speed must be greater than 0.";
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
          setError(errorMessage(loadError));
        }
      });

    rememberApi
      .subscribeToState((nextState) => {
        setState(nextState);
        setError("");
      })
      .then((nextUnsubscribe) => {
        unsubscribe = nextUnsubscribe;
        if (disposed) {
          unsubscribe();
        }
      })
      .catch((subscribeError: unknown) => {
        if (!disposed) {
          setError(errorMessage(subscribeError));
        }
      });

    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, []);

  async function applyState(action: () => Promise<UiState>) {
    try {
      setError("");
      setState(await action());
    } catch (actionError) {
      setError(errorMessage(actionError));
    }
  }

  function handleRecord() {
    void applyState(
      state.mode === "recording" ? rememberApi.stopRecording : rememberApi.startRecording
    );
  }

  function handlePlay() {
    if (validationError) {
      setError(validationError);
      return;
    }
    void applyState(() => rememberApi.startPlayback(loopCount, speedMultiplier));
  }

  function handleStop() {
    void applyState(
      state.mode === "recording" ? rememberApi.stopRecording : rememberApi.stopPlayback
    );
  }

  function handleSave() {
    setError("Save dialog wiring follows the command layer.");
  }

  function handleOpen() {
    setError("Open dialog wiring follows the command layer.");
  }

  const displayedError = validationError || error;

  return (
    <main className="app-shell">
      <header className="app-header">
        <div>
          <h1>Remember</h1>
          <p>Mode: {state.mode}</p>
        </div>
        <p className="mode-summary">{state.message}</p>
      </header>
      <div className="content-grid">
        <div className="main-stack">
          <Controls
            state={state}
            hasRecording={hasRecording}
            onRecord={handleRecord}
            onPlay={handlePlay}
            onStop={handleStop}
            onSave={handleSave}
            onOpen={handleOpen}
          />
          <PlaybackSettings
            loopCount={loopCount}
            speedMultiplier={speedMultiplier}
            onLoopCountChange={setLoopCount}
            onSpeedMultiplierChange={setSpeedMultiplier}
          />
          <StatusPanel state={state} error={displayedError} />
        </div>
        <div className="side-stack">
          <HotkeyPanel />
        </div>
      </div>
    </main>
  );
}
