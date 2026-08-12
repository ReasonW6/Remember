import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

const warningEvent = "remember://capture-warning";

interface CaptureWarningPayload {
  message: string;
}

export function CaptureWarning() {
  const [message, setMessage] = useState("");

  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | undefined;

    async function subscribe() {
      try {
        const nextUnsubscribe = await listen<CaptureWarningPayload>(warningEvent, (event) => {
          if (!disposed) {
            setMessage(event.payload.message);
          }
        });
        if (disposed) {
          nextUnsubscribe();
        } else {
          unsubscribe = nextUnsubscribe;
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

  return message ? (
    <div className="capture-warning" role="status" aria-live="polite">
      {message}
    </div>
  ) : null;
}
