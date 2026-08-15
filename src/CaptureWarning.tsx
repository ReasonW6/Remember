import { useEffect, useState } from "react";
import { emit, listen } from "@tauri-apps/api/event";

const warningEvent = "remember://capture-warning";
const warningReadyEvent = "remember://capture-warning-ready";

interface CaptureWarningPayload {
  kind: "capture-unavailable" | "playback-waiting" | "playback-stopped";
  title: string;
  message: string;
  detail: string;
}

export function CaptureWarning() {
  const [notice, setNotice] = useState<CaptureWarningPayload | null>(null);

  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | undefined;

    async function subscribe() {
      try {
        const nextUnsubscribe = await listen<CaptureWarningPayload>(warningEvent, (event) => {
          if (!disposed) {
            setNotice(event.payload);
          }
        });
        if (disposed) {
          nextUnsubscribe();
        } else {
          unsubscribe = nextUnsubscribe;
          await emit(warningReadyEvent);
        }
      } catch {
        // This warning is supplementary; a missing event permission must not
        // create an unhandled rejection in its transparent window.
      }
    }

    void subscribe();

    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, []);

  return notice ? (
    <div
      className={`capture-warning capture-warning-${notice.kind}`}
      role={notice.kind === "playback-stopped" ? "alert" : "status"}
      aria-live={notice.kind === "playback-stopped" ? "assertive" : "polite"}
    >
      <span className="capture-warning-signal" aria-hidden="true" />
      <span className="capture-warning-copy">
        <strong>{notice.title}</strong>
        <span>{notice.message}</span>
        <small>{notice.detail}</small>
      </span>
    </div>
  ) : null;
}
