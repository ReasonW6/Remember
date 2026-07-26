import { act, render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ActivityIndicator } from "./ActivityIndicator";
import type { UiState } from "./types";

const apiMocks = vi.hoisted(() => ({
  getState: vi.fn(),
  subscribeToState: vi.fn()
}));

vi.mock("./lib/rememberApi", () => apiMocks);

const recordingState: UiState = {
  mode: "recording",
  recording_name: null,
  step_count: 0,
  duration_ms: 0,
  message: "Recording",
  message_is_error: false,
  revision: 1
};

describe("ActivityIndicator", () => {
  let stateListener: ((state: UiState) => void) | undefined;

  beforeEach(() => {
    vi.clearAllMocks();
    stateListener = undefined;
    apiMocks.getState.mockResolvedValue(recordingState);
    apiMocks.subscribeToState.mockImplementation(
      async (listener: (state: UiState) => void) => {
        stateListener = listener;
        return () => undefined;
      }
    );
  });

  it("uses a green dot while recording and a blue dot while playing", async () => {
    const { container } = render(<ActivityIndicator />);

    await waitFor(() =>
      expect(container.querySelector(".activity-indicator-recording")).toBeInTheDocument()
    );

    act(() => {
      stateListener?.({ ...recordingState, mode: "playing", revision: 2 });
    });

    expect(container.querySelector(".activity-indicator-playing")).toBeInTheDocument();
  });

  it("hides the dot in idle mode and ignores stale state", async () => {
    const { container } = render(<ActivityIndicator />);

    await waitFor(() => expect(stateListener).toBeTypeOf("function"));
    act(() => {
      stateListener?.({ ...recordingState, mode: "playing", revision: 3 });
      stateListener?.({ ...recordingState, mode: "idle", revision: 2 });
    });

    expect(container.querySelector(".activity-indicator-playing")).toBeInTheDocument();

    act(() => {
      stateListener?.({ ...recordingState, mode: "idle", revision: 4 });
    });

    expect(container.querySelector(".activity-indicator-dot")).not.toBeInTheDocument();
  });
});
