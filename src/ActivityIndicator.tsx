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

    void rememberApi
      .subscribeToState(applyState)
      .then((nextUnsubscribe) => {
        unsubscribe = nextUnsubscribe;
        if (disposed) {
          unsubscribe();
        }
      })
      .catch(() => undefined);

    void rememberApi.getState().then(applyState).catch(() => undefined);

    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, []);

  if (mode === "idle") {
    return null;
  }

  return (
    <div className={`activity-indicator activity-indicator-${mode}`} aria-hidden="true">
      <span className="activity-indicator-dot" />
    </div>
  );
}
