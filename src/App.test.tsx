import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";

const apiMocks = vi.hoisted(() => ({
  getState: vi.fn(),
  startRecording: vi.fn(),
  stopRecording: vi.fn(),
  startPlayback: vi.fn(),
  stopPlayback: vi.fn(),
  subscribeToState: vi.fn()
}));

vi.mock("./lib/rememberApi", () => apiMocks);

const idleState = {
  mode: "idle",
  recording_name: null,
  step_count: 0,
  duration_ms: 0,
  message: "Idle"
};

const recordingState = {
  mode: "recording",
  recording_name: null,
  step_count: 0,
  duration_ms: 0,
  message: "Recording"
};

const playingState = {
  mode: "playing",
  recording_name: "demo",
  step_count: 3,
  duration_ms: 1200,
  message: "Playing"
};

const stoppedState = {
  mode: "idle",
  recording_name: "demo",
  step_count: 3,
  duration_ms: 1200,
  message: "Playback stopped"
};

describe("App", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    apiMocks.getState.mockResolvedValue(idleState);
    apiMocks.subscribeToState.mockResolvedValue(() => undefined);
    apiMocks.startRecording.mockResolvedValue(recordingState);
    apiMocks.stopRecording.mockResolvedValue(stoppedState);
    apiMocks.startPlayback.mockResolvedValue(playingState);
    apiMocks.stopPlayback.mockResolvedValue(stoppedState);
  });

  it("renders idle controls and hotkeys", async () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "Remember" })).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "Record" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Play" })).toBeDisabled();
    expect(screen.getByText("Ctrl+Alt+R")).toBeInTheDocument();

    await waitFor(() => expect(apiMocks.getState).toHaveBeenCalledTimes(1));
    expect(apiMocks.subscribeToState).toHaveBeenCalledWith(expect.any(Function));
  });

  it("starts recording from the Record button", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Record" }));

    await waitFor(() => expect(apiMocks.startRecording).toHaveBeenCalledTimes(1));
    expect(await screen.findAllByText("Recording")).not.toHaveLength(0);
  });

  it("stops playback from the Stop button", async () => {
    apiMocks.getState.mockResolvedValue(playingState);
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => expect(screen.getByRole("button", { name: "Stop" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "Stop" }));

    await waitFor(() => expect(apiMocks.stopPlayback).toHaveBeenCalledTimes(1));
    expect(await screen.findAllByText("Playback stopped")).not.toHaveLength(0);
  });

  it("validates loop count", async () => {
    const user = userEvent.setup();
    render(<App />);

    const loopCount = await screen.findByLabelText("Loop count");
    await user.clear(loopCount);
    await user.type(loopCount, "0");

    expect(screen.getByRole("alert")).toHaveTextContent("Loop count must be at least 1.");
  });
});
