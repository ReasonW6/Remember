import { AlertTriangle, AppWindow, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import * as rememberApi from "./lib/rememberApi";
import { displayErrorMessage } from "./localization";
import type { WindowBindingCandidate, WindowBindingRequest } from "./types";

function executableName(path: string) {
  const segments = path.split(/[\\/]/).filter(Boolean);
  return segments[segments.length - 1] ?? path;
}

function windowLabel(candidate: WindowBindingCandidate) {
  return candidate.title.trim() || executableName(candidate.executable_path) || "未命名窗口";
}

export function WindowBinding() {
  const [request, setRequest] = useState<WindowBindingRequest | null>(null);
  const [pendingCandidate, setPendingCandidate] = useState<number | null>(null);
  const [error, setError] = useState("");
  const requestIdRef = useRef<number | null>(null);
  const hoveredCandidateRef = useRef<number | null>(null);
  const focusedCandidateRef = useRef<number | null>(null);
  const highlightQueueRef = useRef<Promise<void>>(Promise.resolve());

  function queueHighlight(requestId: number, candidateId: number | null) {
    const nextHighlight = highlightQueueRef.current
      .catch(() => undefined)
      .then(() => rememberApi.highlightWindowBindingCandidate(requestId, candidateId));
    highlightQueueRef.current = nextHighlight.catch(() => undefined);
  }

  function clearCandidateInteraction(requestId: number) {
    hoveredCandidateRef.current = null;
    focusedCandidateRef.current = null;
    queueHighlight(requestId, null);
  }

  function presentRequest(nextRequest: WindowBindingRequest) {
    const previousRequestId = requestIdRef.current;
    if (previousRequestId !== null) {
      clearCandidateInteraction(previousRequestId);
    }
    requestIdRef.current = nextRequest.request_id;
    setRequest(nextRequest);
    setPendingCandidate(null);
    setError("");
  }

  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | undefined;
    let receivedEvent = false;

    async function initialize() {
      try {
        const nextUnsubscribe = await rememberApi.subscribeToWindowBinding((nextRequest) => {
          if (!disposed) {
            receivedEvent = true;
            presentRequest(nextRequest);
          }
        });
        if (disposed) {
          nextUnsubscribe();
          return;
        }
        unsubscribe = nextUnsubscribe;
        const pending = await rememberApi.getPendingWindowBinding();
        if (!disposed && !receivedEvent && pending) {
          presentRequest(pending);
        }
      } catch (loadError) {
        if (!disposed && !receivedEvent) {
          setError(displayErrorMessage(loadError));
        }
      }
    }

    void initialize();
    return () => {
      disposed = true;
      unsubscribe?.();
      const activeRequestId = requestIdRef.current;
      requestIdRef.current = null;
      if (activeRequestId !== null) {
        clearCandidateInteraction(activeRequestId);
      }
    };
  }, []);

  async function selectCandidate(candidateId: number) {
    if (!request || pendingCandidate !== null) {
      return;
    }
    const requestId = request.request_id;
    clearCandidateInteraction(requestId);
    setPendingCandidate(candidateId);
    setError("");
    try {
      await rememberApi.selectWindowBindingCandidate(requestId, candidateId);
    } catch (selectionError) {
      if (requestIdRef.current === requestId) {
        setPendingCandidate(null);
        setError(displayErrorMessage(selectionError));
      }
    }
  }

  async function cancel() {
    if (!request || pendingCandidate !== null) {
      return;
    }
    const requestId = request.request_id;
    clearCandidateInteraction(requestId);
    setPendingCandidate(-1);
    setError("");
    try {
      await rememberApi.cancelWindowBinding(requestId);
    } catch (cancelError) {
      if (requestIdRef.current === requestId) {
        setPendingCandidate(null);
        setError(displayErrorMessage(cancelError));
      }
    }
  }

  function updateHighlight() {
    const requestId = requestIdRef.current;
    if (requestId === null) {
      return;
    }
    queueHighlight(requestId, hoveredCandidateRef.current ?? focusedCandidateRef.current);
  }

  return (
    <main className="window-binding-shell">
      <header className="window-binding-titlebar" data-tauri-drag-region>
        <div className="window-binding-title" data-tauri-drag-region>
          <AppWindow size={17} aria-hidden="true" />
          <span data-tauri-drag-region>选择目标窗口</span>
        </div>
        <button
          type="button"
          className="window-binding-close"
          aria-label="取消回放"
          disabled={!request || pendingCandidate !== null}
          onClick={() => void cancel()}
        >
          <X size={17} aria-hidden="true" />
        </button>
      </header>

      <section className="window-binding-content">
        {request ? (
          <>
            <div className="window-binding-explanation">
              <AlertTriangle size={18} aria-hidden="true" />
              <div>
                <h1>需要确认目标窗口</h1>
                <p>将鼠标停在候选项上可在桌面高亮窗口，然后选择正确的一个。</p>
              </div>
            </div>

            <dl className="window-binding-recorded" aria-label="录制中的目标窗口">
              <div>
                <dt>录制标题</dt>
                <dd title={request.target.title || "（无标题）"}>
                  {request.target.title || "（无标题）"}
                </dd>
              </div>
              <div>
                <dt>程序</dt>
                <dd title={request.target.executable_path}>
                  {executableName(request.target.executable_path)}
                </dd>
              </div>
              <div>
                <dt>客户区</dt>
                <dd>
                  {request.target.client_size.width} × {request.target.client_size.height} · DPI {request.target.dpi}
                </dd>
              </div>
            </dl>

            <ul
              className="window-binding-list"
              aria-label="候选窗口"
              aria-busy={pendingCandidate !== null}
            >
              {request.candidates.map((candidate) => (
                <li key={candidate.candidate_id} className="window-binding-item">
                  <button
                    type="button"
                    className="window-binding-candidate"
                    disabled={pendingCandidate !== null}
                    onMouseEnter={() => {
                      hoveredCandidateRef.current = candidate.candidate_id;
                      updateHighlight();
                    }}
                    onMouseLeave={() => {
                      if (hoveredCandidateRef.current === candidate.candidate_id) {
                        hoveredCandidateRef.current = null;
                      }
                      updateHighlight();
                    }}
                    onFocus={() => {
                      focusedCandidateRef.current = candidate.candidate_id;
                      updateHighlight();
                    }}
                    onBlur={() => {
                      if (focusedCandidateRef.current === candidate.candidate_id) {
                        focusedCandidateRef.current = null;
                      }
                      updateHighlight();
                    }}
                    onClick={() => void selectCandidate(candidate.candidate_id)}
                  >
                    <span className="window-binding-candidate-title" title={windowLabel(candidate)}>
                      {windowLabel(candidate)}
                    </span>
                    <span className="window-binding-candidate-meta">
                      {executableName(candidate.executable_path)} · PID {candidate.process_id} · {candidate.client_size.width} × {candidate.client_size.height} · DPI {candidate.dpi}
                    </span>
                    <span
                      className="window-binding-candidate-path"
                      title={candidate.executable_path}
                    >
                      {candidate.executable_path}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
            {pendingCandidate !== null ? (
              <span className="sr-only" role="status">
                {pendingCandidate === -1 ? "正在取消回放" : "正在绑定所选窗口"}
              </span>
            ) : null}
          </>
        ) : (
          <p className="window-binding-waiting" role="status">
            正在等待窗口绑定请求…
          </p>
        )}

        {error ? (
          <p className="alert window-binding-error" role="alert">
            {error}
          </p>
        ) : null}
      </section>
    </main>
  );
}
