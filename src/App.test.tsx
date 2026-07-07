import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import type { UiState } from "./types";

const apiMocks = vi.hoisted(() => ({
  getState: vi.fn(),
  startRecording: vi.fn(),
  stopRecording: vi.fn(),
  listRecordings: vi.fn(),
  deleteRecording: vi.fn(),
  setPlaybackSettings: vi.fn(),
  startPlayback: vi.fn(),
  stopPlayback: vi.fn(),
  openRecording: vi.fn(),
  loadRecording: vi.fn(),
  saveCurrentRecording: vi.fn(),
  getHotkeys: vi.fn(),
  setHotkeys: vi.fn(),
  subscribeToState: vi.fn()
}));

const soundMocks = vi.hoisted(() => ({
  playFeedbackTone: vi.fn()
}));

const windowMocks = vi.hoisted(() => ({
  startDragging: vi.fn(),
  minimize: vi.fn(),
  close: vi.fn()
}));

vi.mock("./lib/rememberApi", () => apiMocks);
vi.mock("./lib/sounds", () => soundMocks);
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => windowMocks
}));

const idleState: UiState = {
  mode: "idle",
  recording_name: null,
  step_count: 0,
  duration_ms: 0,
  message: "Idle"
};

const recordingState: UiState = {
  mode: "recording",
  recording_name: null,
  step_count: 0,
  duration_ms: 0,
  message: "Recording"
};

const playingState: UiState = {
  mode: "playing",
  recording_name: "demo",
  step_count: 3,
  duration_ms: 1200,
  message: "Playing"
};

const stoppedState: UiState = {
  mode: "idle",
  recording_name: "demo",
  step_count: 3,
  duration_ms: 1200,
  message: "Playback stopped"
};

const finishedState: UiState = {
  ...stoppedState,
  message: "Playback finished"
};

const recordingFile = {
  name: "demo-auto",
  path: "C:\\Users\\WangXuan\\AppData\\Roaming\\com.remember.desktop\\recordings\\demo-auto.remember.json",
  step_count: 3,
  duration_ms: 1200,
  created_at: "2026-07-01T00:00:00Z",
  updated_at_ms: 1782864000000
};

const defaultHotkeys = {
  record: "F8",
  playback: "F12",
  stop: "F8"
};

describe("App", () => {
  let stateListener: ((state: UiState) => void) | undefined;

  beforeEach(() => {
    vi.clearAllMocks();
    stateListener = undefined;
    apiMocks.getState.mockResolvedValue(idleState);
    apiMocks.listRecordings.mockResolvedValue([]);
    apiMocks.deleteRecording.mockResolvedValue(undefined);
    apiMocks.setPlaybackSettings.mockResolvedValue(undefined);
    apiMocks.getHotkeys.mockResolvedValue(defaultHotkeys);
    apiMocks.setHotkeys.mockResolvedValue(defaultHotkeys);
    apiMocks.subscribeToState.mockImplementation(async (listener: (state: UiState) => void) => {
      stateListener = listener;
      return () => undefined;
    });
    apiMocks.startRecording.mockResolvedValue(recordingState);
    apiMocks.stopRecording.mockResolvedValue(stoppedState);
    apiMocks.startPlayback.mockResolvedValue(playingState);
    apiMocks.stopPlayback.mockResolvedValue(stoppedState);
    apiMocks.openRecording.mockResolvedValue(null);
    apiMocks.loadRecording.mockResolvedValue(stoppedState);
    apiMocks.saveCurrentRecording.mockResolvedValue(undefined);
    windowMocks.startDragging.mockResolvedValue(undefined);
    windowMocks.minimize.mockResolvedValue(undefined);
    windowMocks.close.mockResolvedValue(undefined);
  });

  it("renders idle controls and hotkeys", async () => {
    render(<App />);

    expect(screen.getByRole("toolbar", { name: "窗口控制" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "最小化" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "关闭" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Remember" })).toBeInTheDocument();
    expect(screen.getByAltText("Remember 图标")).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "录制" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "播放" })).toBeDisabled();
    expect(screen.queryByRole("button", { name: "停止" })).not.toBeInTheDocument();
    expect(screen.getByText("模式：就绪")).toBeInTheDocument();
    expect(screen.getAllByText("F8", { selector: "kbd" })).toHaveLength(2);
    expect(screen.getByText("F12", { selector: "kbd" })).toBeInTheDocument();
    expect(screen.getByText("快捷键")).toBeInTheDocument();
    expect(screen.getByText("暂无录制文件")).toBeInTheDocument();
    expect(screen.getAllByRole("heading", { level: 2 }).map((heading) => heading.textContent)).toEqual([
      "录制文件",
      "回放设置",
      "状态",
      "快捷键"
    ]);

    await waitFor(() => expect(apiMocks.getState).toHaveBeenCalledTimes(1));
    expect(apiMocks.listRecordings).toHaveBeenCalledTimes(1);
    expect(apiMocks.getHotkeys).toHaveBeenCalledTimes(1);
    expect(apiMocks.subscribeToState).toHaveBeenCalledWith(expect.any(Function));
  });

  it("handles custom titlebar window controls", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "最小化" }));
    await user.click(screen.getByRole("button", { name: "关闭" }));

    expect(windowMocks.minimize).toHaveBeenCalledTimes(1);
    expect(windowMocks.close).toHaveBeenCalledTimes(1);
  });

  it("starts recording from the Record button", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "录制" }));

    await waitFor(() => expect(apiMocks.startRecording).toHaveBeenCalledTimes(1));
    expect(await screen.findAllByText("正在录制")).not.toHaveLength(0);
    expect(screen.getByRole("button", { name: "停止" })).toBeEnabled();
  });

  it("uses the merged Record button as Stop while recording", async () => {
    apiMocks.getState.mockResolvedValue(recordingState);
    const user = userEvent.setup();
    render(<App />);

    const stop = await screen.findByRole("button", { name: "停止" });
    expect(screen.queryByRole("button", { name: "录制" })).not.toBeInTheDocument();
    await user.click(stop);

    await waitFor(() => expect(apiMocks.stopRecording).toHaveBeenCalledTimes(1));
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

  it("returns to idle when playback finishes from the state event", async () => {
    apiMocks.getState.mockResolvedValue(stoppedState);
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => expect(screen.getByRole("button", { name: "播放" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "播放" }));
    await waitFor(() => expect(apiMocks.startPlayback).toHaveBeenCalledTimes(1));

    act(() => {
      stateListener?.(finishedState);
    });

    expect(await screen.findAllByText("回放完成")).not.toHaveLength(0);
    expect(screen.getByRole("button", { name: "播放" })).toBeEnabled();
    expect(screen.queryByRole("button", { name: "停止" })).not.toBeInTheDocument();
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

  it("syncs playback settings and uses them for focused app playback hotkey", async () => {
    apiMocks.getState.mockResolvedValue(stoppedState);
    const user = userEvent.setup();
    render(<App />);

    const loopCount = await screen.findByLabelText("循环次数");
    await user.clear(loopCount);
    await user.type(loopCount, "3");
    const speed = screen.getByLabelText("速度");
    await user.clear(speed);
    await user.type(speed, "2");
    await waitFor(() => expect(apiMocks.setPlaybackSettings).toHaveBeenCalledWith(3, 2));

    await user.keyboard("{F12}");

    await waitFor(() => expect(apiMocks.startPlayback).toHaveBeenCalledWith(3, 2));
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

  it("loads a recording selected from the saved recording list", async () => {
    apiMocks.listRecordings.mockResolvedValue([recordingFile]);
    const loadedState = {
      ...stoppedState,
      recording_name: "demo-auto",
      message: "Recording loaded"
    };
    apiMocks.loadRecording.mockResolvedValue(loadedState);
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "选择 demo-auto" }));

    await waitFor(() => expect(apiMocks.loadRecording).toHaveBeenCalledWith(recordingFile.path));
    expect(await screen.findAllByText("demo-auto")).not.toHaveLength(0);
  });

  it("deletes a recording from the saved recording list", async () => {
    apiMocks.listRecordings
      .mockResolvedValueOnce([recordingFile])
      .mockResolvedValueOnce([]);
    const user = userEvent.setup();
    render(<App />);

    const deleteButton = await screen.findByRole("button", { name: "删除 demo-auto" });
    expect(deleteButton).toHaveClass("recording-delete-button");
    expect(deleteButton).not.toHaveClass("danger-button");
    await user.click(deleteButton);

    await waitFor(() => expect(apiMocks.deleteRecording).toHaveBeenCalledWith(recordingFile.path));
    await waitFor(() => expect(apiMocks.listRecordings).toHaveBeenCalledTimes(2));
    expect(screen.getByText("暂无录制文件")).toBeInTheDocument();
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

  it("saves custom hotkeys", async () => {
    const nextHotkeys = { record: "Ctrl+Shift+R", playback: "F12", stop: "Ctrl+Shift+R" };
    apiMocks.setHotkeys.mockResolvedValue(nextHotkeys);
    const user = userEvent.setup();
    render(<App />);

    const recordHotkey = await screen.findByLabelText("录制快捷键");
    expect(screen.queryByRole("textbox", { name: "录制快捷键" })).not.toBeInTheDocument();
    await user.click(recordHotkey);
    await user.keyboard("{Control>}{Shift>}r{/Shift}{/Control}");
    const stopHotkey = screen.getByLabelText("停止快捷键");
    await user.click(stopHotkey);
    await user.keyboard("{Control>}{Shift>}r{/Shift}{/Control}");
    await user.click(screen.getByRole("button", { name: "保存快捷键" }));

    await waitFor(() => expect(apiMocks.setHotkeys).toHaveBeenCalledWith(nextHotkeys));
    expect(await screen.findAllByText("Ctrl+Shift+R", { selector: "kbd" })).toHaveLength(2);
  });

  it("does not capture keys on a focused hotkey button before capture starts", async () => {
    const user = userEvent.setup();
    render(<App />);

    const recordHotkey = await screen.findByLabelText("录制快捷键");
    act(() => {
      recordHotkey.focus();
    });
    await user.keyboard("a");

    expect(recordHotkey).toHaveTextContent("F8");
  });

  it("shows plugin ACL errors in Chinese", async () => {
    apiMocks.openRecording.mockRejectedValue(
      new Error("Command plugin:dialog|open not allowed by ACL")
    );
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "打开" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "没有权限打开文件选择窗口，请重启应用后再试。"
    );
  });

  it("plays feedback sounds for recording and playback transitions", async () => {
    render(<App />);
    await waitFor(() => expect(apiMocks.subscribeToState).toHaveBeenCalled());

    act(() => {
      stateListener?.(recordingState);
    });
    act(() => {
      stateListener?.({ ...stoppedState, message: "Recording stopped" });
    });
    act(() => {
      stateListener?.(playingState);
    });
    act(() => {
      stateListener?.(finishedState);
    });

    expect(soundMocks.playFeedbackTone).toHaveBeenCalledWith("recording_start");
    expect(soundMocks.playFeedbackTone).toHaveBeenCalledWith("recording_stop");
    expect(soundMocks.playFeedbackTone).toHaveBeenCalledWith("playback_start");
    expect(soundMocks.playFeedbackTone).toHaveBeenCalledWith("playback_stop");
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
