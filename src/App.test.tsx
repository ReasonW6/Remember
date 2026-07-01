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
  openRecording: vi.fn(),
  saveCurrentRecording: vi.fn(),
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
    apiMocks.openRecording.mockResolvedValue(null);
    apiMocks.saveCurrentRecording.mockResolvedValue(undefined);
  });

  it("renders idle controls and hotkeys", async () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "Remember" })).toBeInTheDocument();
    expect(screen.getByAltText("Remember 图标")).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "录制" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "播放" })).toBeDisabled();
    expect(screen.getByText("模式：就绪")).toBeInTheDocument();
    expect(screen.getByText("Ctrl+Alt+R")).toBeInTheDocument();
    expect(screen.getByText("快捷键")).toBeInTheDocument();

    await waitFor(() => expect(apiMocks.getState).toHaveBeenCalledTimes(1));
    expect(apiMocks.subscribeToState).toHaveBeenCalledWith(expect.any(Function));
  });

  it("starts recording from the Record button", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "录制" }));

    await waitFor(() => expect(apiMocks.startRecording).toHaveBeenCalledTimes(1));
    expect(await screen.findAllByText("正在录制")).not.toHaveLength(0);
  });

  it("stops playback from the Stop button", async () => {
    apiMocks.getState.mockResolvedValue(playingState);
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => expect(screen.getByRole("button", { name: "停止" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "停止" }));

    await waitFor(() => expect(apiMocks.stopPlayback).toHaveBeenCalledTimes(1));
    expect(await screen.findAllByText("回放已停止")).not.toHaveLength(0);
  });

  it("validates loop count", async () => {
    const user = userEvent.setup();
    render(<App />);

    const loopCount = await screen.findByLabelText("循环次数");
    await user.clear(loopCount);
    await user.type(loopCount, "0");

    expect(screen.getByRole("alert")).toHaveTextContent("循环次数必须是大于等于 1 的整数。");
  });

  it("does not start playback with a fractional loop count", async () => {
    apiMocks.getState.mockResolvedValue(stoppedState);
    const user = userEvent.setup();
    render(<App />);

    const loopCount = await screen.findByLabelText("循环次数");
    await user.clear(loopCount);
    await user.type(loopCount, "1.5");
    await user.click(screen.getByRole("button", { name: "播放" }));

    expect(screen.getByRole("alert")).toHaveTextContent("循环次数必须是大于等于 1 的整数。");
    expect(apiMocks.startPlayback).not.toHaveBeenCalled();
  });

  it("does not start playback with a non-finite speed", async () => {
    apiMocks.getState.mockResolvedValue(stoppedState);
    const user = userEvent.setup();
    render(<App />);

    const speed = await screen.findByLabelText("速度");
    await user.clear(speed);
    await user.click(speed);
    await user.paste("1e309");
    await user.click(screen.getByRole("button", { name: "播放" }));

    expect(screen.getByRole("alert")).toHaveTextContent("速度必须是大于 0 的有效数字。");
    expect(apiMocks.startPlayback).not.toHaveBeenCalled();
  });

  it("opens a recording and displays the loaded recording name", async () => {
    const loadedState = {
      mode: "idle",
      recording_name: "loaded.remember.json",
      step_count: 4,
      duration_ms: 2400,
      message: "Loaded recording"
    };
    apiMocks.openRecording.mockResolvedValue(loadedState);
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "打开" }));

    await waitFor(() => expect(apiMocks.openRecording).toHaveBeenCalledTimes(1));
    expect(await screen.findByText("loaded.remember.json")).toBeInTheDocument();
  });

  it("saves the current recording from the Save button", async () => {
    const currentState = {
      mode: "idle",
      recording_name: "current",
      step_count: 2,
      duration_ms: 1500,
      message: "Ready"
    };
    apiMocks.getState.mockResolvedValue(currentState);
    const user = userEvent.setup();
    render(<App />);

    const save = await screen.findByRole("button", { name: "保存" });
    await waitFor(() => expect(save).toBeEnabled());
    await user.click(save);

    await waitFor(() => expect(apiMocks.saveCurrentRecording).toHaveBeenCalledTimes(1));
  });

  it("ignores duplicate record clicks while the recording command is pending", async () => {
    apiMocks.startRecording.mockReturnValue(new Promise(() => undefined));
    const user = userEvent.setup();
    render(<App />);

    const record = await screen.findByRole("button", { name: "录制" });
    await user.dblClick(record);

    expect(apiMocks.startRecording).toHaveBeenCalledTimes(1);
    expect(record).toBeDisabled();
  });
});
