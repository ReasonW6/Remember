import { act, render, screen, waitFor } from "@testing-library/react";
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
    expect(screen.getByRole("status")).toHaveTextContent("正在录制");
    expect(container.querySelector(".activity-indicator-dot")).toBeInTheDocument();
    expect(container.querySelector(".activity-indicator-marker")).not.toBeInTheDocument();

    act(() => {
      stateListener?.({ ...recordingState, mode: "playing", revision: 2 });
    });

    expect(container.querySelector(".activity-indicator-playing")).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("正在回放");
    expect(screen.getByRole("status")).toHaveAttribute("aria-live", "polite");
    expect(container.querySelector(".activity-indicator-dot")).toBeInTheDocument();
    expect(container.querySelector(".activity-indicator-marker")).not.toBeInTheDocument();
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
    expect(container.querySelector(".activity-indicator-marker")).not.toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("就绪");
  });

  it("does not read the snapshot until the state listener is registered", async () => {
    let finishSubscription!: () => void;
    apiMocks.subscribeToState.mockImplementation(
      (listener: (state: UiState) => void) =>
        new Promise<() => void>((resolve) => {
          stateListener = listener;
          finishSubscription = () => resolve(() => undefined);
        })
    );
    render(<ActivityIndicator />);

    await waitFor(() => expect(apiMocks.subscribeToState).toHaveBeenCalledTimes(1));
    expect(apiMocks.getState).not.toHaveBeenCalled();
    act(() => {
      stateListener?.({ ...recordingState, revision: 5 });
      finishSubscription();
    });

    await waitFor(() => expect(apiMocks.getState).toHaveBeenCalledTimes(1));
    expect(screen.getByRole("status")).toHaveTextContent("正在录制");
  });
});
