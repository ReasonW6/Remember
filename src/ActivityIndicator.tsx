import { useEffect, useRef, useState } from "react";
import * as rememberApi from "./lib/rememberApi";
import type { AppMode, UiState } from "./types";

export function ActivityIndicator() {
  const [mode, setMode] = useState<AppMode>("idle");
  const latestRevisionRef = useRef(-1);

  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | undefined;

    function applyState(state: UiState) {
      if (disposed || state.revision < latestRevisionRef.current) {
        return;
      }
      latestRevisionRef.current = state.revision;
      setMode(state.mode);
    }

    async function initialize() {
      try {
        const nextUnsubscribe = await rememberApi.subscribeToState(applyState);
        if (disposed) {
          nextUnsubscribe();
          return;
        }
        unsubscribe = nextUnsubscribe;
      } catch {
        // The indicator is supplementary; a missing event permission must not
        // surface an unusable transparent window.
      }

      if (!disposed) {
        void rememberApi.getState().then(applyState).catch(() => undefined);
      }
    }

    void initialize();

    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, []);

  if (mode === "idle") {
    return (
      <span className="sr-only" role="status" aria-live="polite">
        就绪
      </span>
    );
  }

  const modeLabel = mode === "recording" ? "正在录制" : "正在回放";
  return (
    <div
      className={`activity-indicator activity-indicator-${mode}`}
      role="status"
      aria-live="polite"
    >
      <span className="activity-indicator-dot" aria-hidden="true" />
      <span className="sr-only">{modeLabel}</span>
    </div>
  );
}
