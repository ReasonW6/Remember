import {
  act,
  fireEvent,
  render as renderCompact,
  screen,
  waitFor
} from "@testing-library/react";
import type { ReactElement } from "react";
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
  renameRecording: vi.fn(),
  setPlaybackSettings: vi.fn(),
  startPlayback: vi.fn(),
  stopPlayback: vi.fn(),
  openRecording: vi.fn(),
  loadRecording: vi.fn(),
  saveCurrentRecording: vi.fn(),
  getHotkeys: vi.fn(),
  getAdvancedSettings: vi.fn(),
  showAdvancedSettings: vi.fn(),
  getPrivilegeState: vi.fn(),
  restartAsAdministrator: vi.fn(),
  confirmDeleteRecording: vi.fn(),
  subscribeToState: vi.fn(),
  subscribeToRecordingsChanged: vi.fn(),
  subscribeToHotkeysChanged: vi.fn(),
  subscribeToAdvancedSettingsChanged: vi.fn()
}));

const soundMocks = vi.hoisted(() => ({
  playFeedbackTone: vi.fn()
}));

const windowMocks = vi.hoisted(() => ({
  startDragging: vi.fn(),
  minimize: vi.fn(),
  setSize: vi.fn(),
  close: vi.fn()
}));

vi.mock("./lib/rememberApi", () => apiMocks);
vi.mock("./lib/sounds", () => soundMocks);
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => windowMocks,
  LogicalSize: class {
    width: number;
    height: number;

    constructor(width: number, height: number) {
      this.width = width;
      this.height = height;
    }
  }
}));

function render(ui: ReactElement) {
  const result = renderCompact(ui);
  const expand = screen.queryByRole("button", { name: "展开完整界面" });
  if (expand) {
    fireEvent.click(expand);
  }
  return result;
}

const idleState: UiState = {
  mode: "idle",
  recording_name: null,
  step_count: 0,
  duration_ms: 0,
  message: "Idle",
  revision: 1,
  message_is_error: false
};

const recordingState: UiState = {
  mode: "recording",
  recording_name: null,
  step_count: 0,
  duration_ms: 0,
  message: "Recording",
  revision: 2,
  message_is_error: false
};

const playingState: UiState = {
  mode: "playing",
  recording_name: "demo",
  step_count: 3,
  duration_ms: 1200,
  message: "Playing",
  revision: 3,
  message_is_error: false
};

const stoppedState: UiState = {
  mode: "idle",
  recording_name: "demo",
  step_count: 3,
  duration_ms: 1200,
  message: "Playback stopped",
  revision: 4,
  message_is_error: false
};

const finishedState: UiState = {
  ...stoppedState,
  message: "Playback finished",
  revision: 5
};

const recordingFile = {
  name: "demo-auto",
  path: "C:\\Users\\WangXuan\\AppData\\Roaming\\com.remember.desktop\\recordings\\demo-auto.remember.json",
  step_count: 3,
  duration_ms: 1200,
  created_at: "2026-07-01T00:00:00Z",
  updated_at_ms: 1782864000000,
  load_error: null
};

const defaultHotkeys = {
  record: "F8",
  playback: "F12",
  stop: "F8"
};

describe("App", () => {
  let stateListener: ((state: UiState) => void) | undefined;
  let recordingsChangedListener: (() => void) | undefined;
  let hotkeysChangedListener: ((hotkeys: typeof defaultHotkeys) => void) | undefined;
  let advancedSettingsChangedListener:
    | ((settings: {
        feedback_volume_percent: number;
        feedback_muted: boolean;
        show_activity_indicator: boolean;
      }) => void)
    | undefined;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal(
      "requestAnimationFrame",
      vi.fn((callback: FrameRequestCallback) =>
        window.setTimeout(() => callback(window.performance.now()), 0)
      )
    );
    vi.stubGlobal(
      "cancelAnimationFrame",
      vi.fn((handle: number) => window.clearTimeout(handle))
    );
    window.localStorage.clear();
    stateListener = undefined;
    recordingsChangedListener = undefined;
    hotkeysChangedListener = undefined;
    advancedSettingsChangedListener = undefined;
    apiMocks.getState.mockResolvedValue(idleState);
    apiMocks.listRecordings.mockResolvedValue([]);
    apiMocks.deleteRecording.mockResolvedValue(undefined);
    apiMocks.renameRecording.mockResolvedValue(
      "C:\\Users\\WangXuan\\AppData\\Roaming\\com.remember.desktop\\recordings\\renamed.remember.json"
    );
    apiMocks.setPlaybackSettings.mockResolvedValue(undefined);
    apiMocks.getHotkeys.mockResolvedValue(defaultHotkeys);
    apiMocks.getAdvancedSettings.mockResolvedValue({
      feedback_volume_percent: 50,
      feedback_muted: false,
      show_activity_indicator: true
    });
    apiMocks.showAdvancedSettings.mockResolvedValue(undefined);
    apiMocks.getPrivilegeState.mockResolvedValue({ is_elevated: false });
    apiMocks.restartAsAdministrator.mockResolvedValue(undefined);
    apiMocks.confirmDeleteRecording.mockResolvedValue(true);
    apiMocks.subscribeToState.mockImplementation(async (listener: (state: UiState) => void) => {
      stateListener = listener;
      return () => undefined;
    });
    apiMocks.subscribeToRecordingsChanged.mockImplementation(
      async (listener: () => void) => {
        recordingsChangedListener = listener;
        return () => undefined;
      }
    );
    apiMocks.subscribeToHotkeysChanged.mockImplementation(
      async (listener: (hotkeys: typeof defaultHotkeys) => void) => {
        hotkeysChangedListener = listener;
        return () => undefined;
      }
    );
    apiMocks.subscribeToAdvancedSettingsChanged.mockImplementation(
      async (listener: typeof advancedSettingsChangedListener) => {
        advancedSettingsChangedListener = listener;
        return () => undefined;
      }
    );
    apiMocks.startRecording.mockResolvedValue(recordingState);
    apiMocks.stopRecording.mockResolvedValue(stoppedState);
    apiMocks.startPlayback.mockResolvedValue(playingState);
    apiMocks.stopPlayback.mockResolvedValue(stoppedState);
    apiMocks.openRecording.mockResolvedValue(null);
    apiMocks.loadRecording.mockResolvedValue(stoppedState);
    apiMocks.saveCurrentRecording.mockResolvedValue(undefined);
    windowMocks.startDragging.mockResolvedValue(undefined);
    windowMocks.minimize.mockResolvedValue(undefined);
    windowMocks.setSize.mockResolvedValue(undefined);
    windowMocks.close.mockResolvedValue(undefined);
  });

  it("starts with a compact file, record, and play interface", async () => {
    const { container } = renderCompact(<App />);

    expect(container.querySelector(".window-titlebar")).toHaveTextContent("Remember");
    expect(container.querySelector(".window-titlebar")?.textContent).toBe("Remember");
    expect(screen.getByRole("button", { name: "展开完整界面" })).toBeEnabled();
    expect(screen.getByRole("combobox", { name: "选择录制文件" })).toBeDisabled();
    expect(await screen.findByRole("button", { name: "录制" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "播放" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "以管理员身份重启" })).toBeEnabled();
    for (const button of [
      screen.getByRole("button", { name: "录制" }),
      screen.getByRole("button", { name: "播放" }),
      screen.getByRole("button", { name: "以管理员身份重启" })
    ]) {
      expect(button).toHaveClass("compact-action-button");
    }
    expect(screen.queryByRole("button", { name: "高级设置" })).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "录制文件" })).not.toBeInTheDocument();
  });

  it("starts state synchronization immediately and defers non-critical initialization", async () => {
    let runDeferredInitialization: FrameRequestCallback | undefined;
    vi.mocked(window.requestAnimationFrame).mockImplementation((callback) => {
      runDeferredInitialization = callback;
      return 17;
    });
    renderCompact(<App />);

    await waitFor(() => expect(apiMocks.subscribeToState).toHaveBeenCalledTimes(1));
    expect(apiMocks.subscribeToRecordingsChanged).not.toHaveBeenCalled();
    expect(apiMocks.subscribeToHotkeysChanged).not.toHaveBeenCalled();
    expect(apiMocks.subscribeToAdvancedSettingsChanged).not.toHaveBeenCalled();
    expect(apiMocks.listRecordings).not.toHaveBeenCalled();
    expect(apiMocks.getHotkeys).not.toHaveBeenCalled();
    expect(apiMocks.getAdvancedSettings).not.toHaveBeenCalled();
    expect(apiMocks.getPrivilegeState).not.toHaveBeenCalled();

    act(() => runDeferredInitialization?.(window.performance.now()));

    await waitFor(() => expect(apiMocks.listRecordings).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(apiMocks.getHotkeys).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(apiMocks.getAdvancedSettings).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(apiMocks.getPrivilegeState).toHaveBeenCalledTimes(1));
    expect(apiMocks.subscribeToRecordingsChanged.mock.invocationCallOrder[0]).toBeLessThan(
      apiMocks.listRecordings.mock.invocationCallOrder[0]
    );
    expect(apiMocks.subscribeToHotkeysChanged.mock.invocationCallOrder[0]).toBeLessThan(
      apiMocks.getHotkeys.mock.invocationCallOrder[0]
    );
    expect(apiMocks.subscribeToAdvancedSettingsChanged.mock.invocationCallOrder[0]).toBeLessThan(
      apiMocks.getAdvancedSettings.mock.invocationCallOrder[0]
    );
  });

  it("cancels deferred initialization when the app unmounts before the next frame", () => {
    let deferredInitialization: FrameRequestCallback | undefined;
    vi.mocked(window.requestAnimationFrame).mockImplementation((callback) => {
      deferredInitialization = callback;
      return 23;
    });
    const { unmount } = renderCompact(<App />);

    unmount();

    expect(window.cancelAnimationFrame).toHaveBeenCalledWith(23);
    act(() => deferredInitialization?.(window.performance.now()));
    expect(apiMocks.subscribeToRecordingsChanged).not.toHaveBeenCalled();
    expect(apiMocks.subscribeToHotkeysChanged).not.toHaveBeenCalled();
    expect(apiMocks.subscribeToAdvancedSettingsChanged).not.toHaveBeenCalled();
    expect(apiMocks.getPrivilegeState).not.toHaveBeenCalled();
  });

  it("switches between compact and full window sizes", async () => {
    const user = userEvent.setup();
    renderCompact(<App />);

    await user.click(screen.getByRole("button", { name: "展开完整界面" }));
    await waitFor(() =>
      expect(windowMocks.setSize).toHaveBeenLastCalledWith(
        expect.objectContaining({ width: 420, height: 520 })
      )
    );
    expect(screen.getByRole("button", { name: "切换到小悬浮窗" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "高级设置" })).toBeEnabled();

    await user.click(screen.getByRole("button", { name: "切换到小悬浮窗" }));
    await waitFor(() =>
      expect(windowMocks.setSize).toHaveBeenLastCalledWith(
        expect.objectContaining({ width: 360, height: 134 })
      )
    );
    expect(screen.getByRole("combobox", { name: "选择录制文件" })).toBeInTheDocument();
  });

  it("restores the last selected recording in the compact selector", async () => {
    window.localStorage.setItem("remember:last-recording-path", recordingFile.path);
    apiMocks.listRecordings.mockResolvedValue([recordingFile]);
    apiMocks.loadRecording.mockResolvedValue({
      ...stoppedState,
      recording_name: recordingFile.name,
      revision: 3
    });
    renderCompact(<App />);

    const selector = await screen.findByRole("combobox", { name: "选择录制文件" });
    await waitFor(() => expect(selector).toHaveValue(recordingFile.path));
    expect(apiMocks.loadRecording).toHaveBeenCalledWith(recordingFile.path);
  });

  it("removes the recording prompt after a compact selection", async () => {
    apiMocks.listRecordings.mockResolvedValue([recordingFile]);
    apiMocks.loadRecording.mockResolvedValue({
      ...stoppedState,
      recording_name: recordingFile.name,
      revision: 3
    });
    const user = userEvent.setup();
    renderCompact(<App />);

    const selector = await screen.findByRole("combobox", { name: "选择录制文件" });
    expect(screen.getByRole("option", { name: "选择录制文件" })).toBeDisabled();
    await user.selectOptions(selector, recordingFile.path);

    await waitFor(() => expect(selector).toHaveValue(recordingFile.path));
    expect(screen.queryByRole("option", { name: "选择录制文件" })).not.toBeInTheDocument();
    expect(screen.getByRole("option", { name: recordingFile.name })).toBeInTheDocument();
  });

  it("restarts as administrator from the compact window", async () => {
    const user = userEvent.setup();
    renderCompact(<App />);

    await user.click(await screen.findByRole("button", { name: "以管理员身份重启" }));

    expect(apiMocks.restartAsAdministrator).toHaveBeenCalledTimes(1);
  });

  it("renders idle controls and moves hotkeys into advanced settings", async () => {
    render(<App />);

    expect(screen.getByRole("toolbar", { name: "窗口控制" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "最小化" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "关闭" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Remember" })).toBeInTheDocument();
    expect(screen.getByAltText("Remember 图标")).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "录制" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "播放" })).toBeDisabled();
    expect(screen.queryByRole("button", { name: "停止" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "高级设置" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "以管理员身份重启" })).toBeEnabled();
    const administratorHelp = screen.getByLabelText("管理员模式说明");
    expect(administratorHelp).toHaveAttribute(
      "data-tooltip",
      expect.stringContaining("高权限的系统窗口")
    );
    expect(administratorHelp).not.toHaveAttribute(
      "data-tooltip",
      expect.stringContaining("网络适配器")
    );
    expect(screen.getByText("模式：就绪")).toBeInTheDocument();
    expect(screen.getByText("就绪", { selector: ".mode-summary" })).toHaveAttribute(
      "aria-live",
      "polite"
    );
    expect(screen.queryByText("快捷键")).not.toBeInTheDocument();
    expect(screen.getByText("暂无录制文件")).toBeInTheDocument();
    expect(screen.getAllByRole("heading", { level: 2 }).map((heading) => heading.textContent)).toEqual([
      "录制文件",
      "回放设置",
      "状态"
    ]);

    await waitFor(() => expect(apiMocks.getState).toHaveBeenCalledTimes(1));
    expect(apiMocks.listRecordings).toHaveBeenCalledTimes(1);
    expect(apiMocks.getHotkeys).toHaveBeenCalledTimes(1);
    expect(apiMocks.getAdvancedSettings).toHaveBeenCalledTimes(1);
    expect(apiMocks.getPrivilegeState).toHaveBeenCalledTimes(1);
    expect(apiMocks.subscribeToState).toHaveBeenCalledWith(expect.any(Function));
  });

  it("handles custom titlebar window controls", async () => {
    const user = userEvent.setup();
    const { container } = render(<App />);

    const dragRegion = container.querySelector(".window-titlebar-drag");
    expect(dragRegion).toHaveAttribute("data-tauri-drag-region");
    fireEvent.mouseDown(dragRegion as Element, { button: 0 });
    expect(windowMocks.startDragging).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "最小化" }));
    await user.click(screen.getByRole("button", { name: "关闭" }));

    await waitFor(() => expect(windowMocks.minimize).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(windowMocks.close).toHaveBeenCalledTimes(1));
  });

  it("opens advanced settings from the main control bar", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "高级设置" }));

    expect(apiMocks.showAdvancedSettings).toHaveBeenCalledTimes(1);
  });

  it("requests an administrator restart from the dedicated control", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "以管理员身份重启" }));

    expect(apiMocks.restartAsAdministrator).toHaveBeenCalledTimes(1);
  });

  it("disables administrator restart when already elevated", async () => {
    apiMocks.getPrivilegeState.mockResolvedValue({ is_elevated: true });
    render(<App />);

    expect(await screen.findByRole("button", { name: "已在管理员模式" })).toBeDisabled();
  });

  it("starts recording from the Record button", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "录制" }));

    await waitFor(() => expect(apiMocks.startRecording).toHaveBeenCalledTimes(1));
    expect(windowMocks.minimize).not.toHaveBeenCalled();
    expect(await screen.findAllByText("正在录制")).not.toHaveLength(0);
    expect(screen.getByRole("button", { name: "停止" })).toBeEnabled();
  });

  it("reports a manual recording failure without minimizing the main window", async () => {
    apiMocks.startRecording.mockRejectedValue(new Error("recording unavailable"));
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "录制" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("recording unavailable");
    expect(windowMocks.minimize).not.toHaveBeenCalled();
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

    expect(screen.getByRole("alert")).toHaveTextContent(
      "循环次数必须是 1 到 4294967295 之间的整数。"
    );
    expect(screen.getByText(/当前输入无效/, { selector: ".playback-settings-status" }))
      .toHaveTextContent("前台播放已禁用，全局 F12 仍使用已应用值。");
  });

  it("does not start playback with a fractional loop count", async () => {
    apiMocks.getState.mockResolvedValue(stoppedState);
    const user = userEvent.setup();
    render(<App />);

    const loopCount = await screen.findByLabelText("循环次数");
    await user.clear(loopCount);
    await user.type(loopCount, "1.5");
    expect(screen.getByRole("button", { name: "播放" })).toBeDisabled();

    expect(screen.getByRole("alert")).toHaveTextContent(
      "循环次数必须是 1 到 4294967295 之间的整数。"
    );
    expect(apiMocks.startPlayback).not.toHaveBeenCalled();
  });

  it("does not send loop counts larger than Rust u32", async () => {
    apiMocks.getState.mockResolvedValue(stoppedState);
    render(<App />);

    const loopCount = await screen.findByLabelText("循环次数");
    fireEvent.change(loopCount, { target: { value: "4294967296" } });

    expect(screen.getByRole("button", { name: "播放" })).toBeDisabled();
    expect(screen.getByRole("alert")).toHaveTextContent(
      "循环次数必须是 1 到 4294967295 之间的整数。"
    );
    expect(apiMocks.setPlaybackSettings).not.toHaveBeenCalledWith(4294967296, 1);
  });

  it("does not start playback with a non-finite speed", async () => {
    apiMocks.getState.mockResolvedValue(stoppedState);
    const user = userEvent.setup();
    render(<App />);

    const speed = await screen.findByLabelText("速度");
    await user.clear(speed);
    await user.click(speed);
    await user.paste("1e309");
    expect(screen.getByRole("button", { name: "播放" })).toBeDisabled();

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

  it("blocks UI and focused hotkey playback until settings synchronization is acknowledged", async () => {
    let acknowledgeSettings!: () => void;
    apiMocks.getState.mockResolvedValue(stoppedState);
    apiMocks.setPlaybackSettings.mockReturnValue(
      new Promise<void>((resolve) => {
        acknowledgeSettings = resolve;
      })
    );
    render(<App />);

    const play = await screen.findByRole("button", { name: "播放" });
    expect(play).toBeDisabled();
    expect(screen.getByText(/正在应用新设置/, { selector: ".playback-settings-status" }))
      .toHaveTextContent("全局 F12 仍使用已应用值");
    fireEvent.keyDown(window, { key: "F12" });
    expect(apiMocks.startPlayback).not.toHaveBeenCalled();

    act(() => acknowledgeSettings());

    await waitFor(() => expect(play).toBeEnabled());
    fireEvent.keyDown(window, { key: "F12" });
    await waitFor(() => expect(apiMocks.startPlayback).toHaveBeenCalledWith(1, 1));
  });

  it("serializes playback setting writes and exposes only acknowledged values", async () => {
    const acknowledgements: Array<() => void> = [];
    apiMocks.getState.mockResolvedValue(stoppedState);
    apiMocks.setPlaybackSettings.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          acknowledgements.push(resolve);
        })
    );
    render(<App />);

    await waitFor(() => expect(apiMocks.setPlaybackSettings).toHaveBeenCalledTimes(1));
    fireEvent.change(screen.getByLabelText("循环次数"), { target: { value: "3" } });

    expect(apiMocks.setPlaybackSettings).toHaveBeenCalledTimes(1);
    expect(screen.getByText(/正在应用新设置/, { selector: ".playback-settings-status" }))
      .toHaveTextContent("已应用：循环 1 次，速度 1 倍。");

    act(() => acknowledgements[0]());
    await waitFor(() => expect(apiMocks.setPlaybackSettings).toHaveBeenCalledTimes(2));
    expect(apiMocks.setPlaybackSettings).toHaveBeenNthCalledWith(2, 3, 1);
    expect(screen.getByRole("button", { name: "播放" })).toBeDisabled();

    act(() => acknowledgements[1]());
    await waitFor(() =>
      expect(
        screen.getByText("已应用：循环 3 次，速度 1 倍。", {
          selector: ".playback-settings-status"
        })
      ).toBeInTheDocument()
    );
    expect(screen.getByRole("button", { name: "播放" })).toBeEnabled();
  });

  it("opens a recording and displays the loaded recording name", async () => {
    const loadedState = {
      mode: "idle",
      recording_name: "loaded.remember.json",
      step_count: 4,
      duration_ms: 2400,
      message: "Loaded recording",
      revision: 2,
      message_is_error: false
    };
    apiMocks.openRecording.mockResolvedValue({
      path: "C:\\Recordings\\loaded.remember.json",
      state: loadedState
    });
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

    await waitFor(() =>
      expect(apiMocks.confirmDeleteRecording).toHaveBeenCalledWith(recordingFile.name)
    );
    await waitFor(() => expect(apiMocks.deleteRecording).toHaveBeenCalledWith(recordingFile.path));
    await waitFor(() => expect(apiMocks.listRecordings).toHaveBeenCalledTimes(2));
    expect(screen.getByText("暂无录制文件")).toBeInTheDocument();
  });

  it("deletes immediately with Ctrl and exposes the force-delete hint", async () => {
    apiMocks.listRecordings
      .mockResolvedValueOnce([recordingFile])
      .mockResolvedValueOnce([]);
    render(<App />);

    const deleteButton = await screen.findByRole("button", { name: "删除 demo-auto" });
    expect(deleteButton).toHaveAttribute("data-tooltip", "按住 Ctrl 点击强制删除");
    fireEvent.click(deleteButton, { ctrlKey: true });

    await waitFor(() => expect(apiMocks.deleteRecording).toHaveBeenCalledWith(recordingFile.path));
    expect(apiMocks.confirmDeleteRecording).not.toHaveBeenCalled();
  });

  it("renames a recording from the pencil action", async () => {
    const renamedPath =
      "C:\\Users\\WangXuan\\AppData\\Roaming\\com.remember.desktop\\recordings\\weekly-report.remember.json";
    const renamedRecording = {
      ...recordingFile,
      name: "weekly report",
      path: renamedPath
    };
    apiMocks.renameRecording.mockResolvedValue(renamedPath);
    apiMocks.listRecordings
      .mockResolvedValueOnce([recordingFile])
      .mockResolvedValueOnce([renamedRecording]);
    const user = userEvent.setup();
    render(<App />);

    await user.click(
      await screen.findByRole("button", { name: "重命名 demo-auto" }, { timeout: 3000 })
    );
    const input = screen.getByRole("textbox", { name: "重命名 demo-auto" });
    await user.clear(input);
    await user.type(input, "weekly report");
    await user.click(screen.getByRole("button", { name: "保存 demo-auto 的新名称" }));

    await waitFor(() =>
      expect(apiMocks.renameRecording).toHaveBeenCalledWith(recordingFile.path, "weekly report")
    );
    expect(await screen.findByRole("button", { name: "选择 weekly report" })).toBeInTheDocument();
  });

  it("saves the current recording from the Save button", async () => {
    const currentState = {
      mode: "idle",
      recording_name: "current",
      step_count: 2,
      duration_ms: 1500,
      message: "Ready",
      revision: 2,
      message_is_error: false
    };
    apiMocks.getState.mockResolvedValue(currentState);
    const user = userEvent.setup();
    render(<App />);

    const save = await screen.findByRole("button", { name: "保存" });
    await waitFor(() => expect(save).toBeEnabled());
    await user.click(save);

    await waitFor(() => expect(apiMocks.saveCurrentRecording).toHaveBeenCalledTimes(1));
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

    await act(async () => {
      stateListener?.(recordingState);
      stateListener?.({ ...stoppedState, message: "Recording stopped", revision: 3 });
      stateListener?.({ ...playingState, revision: 4 });
      stateListener?.(finishedState);
      await Promise.resolve();
    });

    expect(soundMocks.playFeedbackTone).toHaveBeenCalledWith("recording_start", 50, false);
    expect(soundMocks.playFeedbackTone).toHaveBeenCalledWith("recording_stop", 50, false);
    expect(soundMocks.playFeedbackTone).toHaveBeenCalledWith("playback_start", 50, false);
    expect(soundMocks.playFeedbackTone).toHaveBeenCalledWith("playback_stop", 50, false);
  });

  it("uses advanced sound settings received from the settings window", async () => {
    render(<App />);
    await waitFor(() =>
      expect(apiMocks.subscribeToAdvancedSettingsChanged).toHaveBeenCalledTimes(1)
    );

    act(() => {
      advancedSettingsChangedListener?.({
        feedback_volume_percent: 80,
        feedback_muted: true,
        show_activity_indicator: false
      });
      stateListener?.(recordingState);
    });

    expect(soundMocks.playFeedbackTone).toHaveBeenCalledWith("recording_start", 80, true);
  });

  it("supports infinite playback with a disabled finite loop input", async () => {
    apiMocks.getState.mockResolvedValue(stoppedState);
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("radio", { name: "无限循环" }));

    expect(screen.getByLabelText("循环次数")).toBeDisabled();
    await waitFor(() => expect(apiMocks.setPlaybackSettings).toHaveBeenCalledWith(null, 1));

    await user.click(screen.getByRole("button", { name: "播放" }));
    await waitFor(() => expect(apiMocks.startPlayback).toHaveBeenCalledWith(null, 1));
  });

  it("uses the playback hotkey as stop while playback is active", async () => {
    apiMocks.getState.mockResolvedValue(playingState);
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => expect(screen.getByRole("button", { name: "停止" })).toBeEnabled());
    await user.keyboard("{F12}");

    await waitFor(() => expect(apiMocks.stopPlayback).toHaveBeenCalledTimes(1));
  });

  it("uses hotkeys received from the advanced settings window", async () => {
    render(<App />);
    await waitFor(() => expect(apiMocks.subscribeToHotkeysChanged).toHaveBeenCalledTimes(1));

    act(() => {
      hotkeysChangedListener?.({ record: "F9", playback: "F12", stop: "F9" });
    });
    fireEvent.keyDown(window, { key: "F9" });

    await waitFor(() => expect(apiMocks.startRecording).toHaveBeenCalledTimes(1));
  });

  it("does not let a late command response overwrite a newer state event", async () => {
    const ready = { ...stoppedState, revision: 10 };
    const started = { ...playingState, revision: 11 };
    const finished = { ...finishedState, revision: 12 };
    let resolveStart!: (state: UiState) => void;
    apiMocks.getState.mockResolvedValue(ready);
    apiMocks.startPlayback.mockReturnValue(
      new Promise<UiState>((resolve) => {
        resolveStart = resolve;
      })
    );
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "播放" }));
    await waitFor(() => expect(apiMocks.startPlayback).toHaveBeenCalledTimes(1));
    act(() => stateListener?.(finished));
    act(() => resolveStart(started));

    expect(await screen.findAllByText("回放完成")).not.toHaveLength(0);
    expect(screen.getByRole("button", { name: "播放" })).toBeEnabled();
  });

  it("does not let the initial snapshot overwrite a newer state event", async () => {
    let resolveSnapshot!: (state: UiState) => void;
    apiMocks.getState.mockReturnValue(
      new Promise<UiState>((resolve) => {
        resolveSnapshot = resolve;
      })
    );
    render(<App />);
    await waitFor(() => expect(apiMocks.subscribeToState).toHaveBeenCalled());

    act(() => stateListener?.({ ...recordingState, revision: 5 }));
    act(() => resolveSnapshot(idleState));

    expect(await screen.findAllByText("正在录制")).not.toHaveLength(0);
  });

  it("waits for asynchronous state subscription registration before reading the snapshot", async () => {
    let finishSubscription!: () => void;
    apiMocks.subscribeToState.mockImplementation(
      (listener: (state: UiState) => void) =>
        new Promise<() => void>((resolve) => {
          stateListener = listener;
          finishSubscription = () => resolve(() => undefined);
        })
    );
    render(<App />);

    await waitFor(() => expect(apiMocks.subscribeToState).toHaveBeenCalledTimes(1));
    expect(apiMocks.getState).not.toHaveBeenCalled();

    act(() => {
      stateListener?.({ ...recordingState, revision: 5 });
      finishSubscription();
    });

    await waitFor(() => expect(apiMocks.getState).toHaveBeenCalledTimes(1));
    expect(await screen.findAllByText("正在录制")).not.toHaveLength(0);
  });

  it("refreshes recordings when the backend library changes", async () => {
    apiMocks.listRecordings.mockResolvedValueOnce([]).mockResolvedValueOnce([recordingFile]);
    render(<App />);
    await waitFor(() => expect(apiMocks.listRecordings).toHaveBeenCalledTimes(1));

    act(() => recordingsChangedListener?.());

    expect(await screen.findByRole("button", { name: "选择 demo-auto" })).toBeEnabled();
    expect(apiMocks.listRecordings).toHaveBeenCalledTimes(2);
  });

  it("refreshes recordings after a recording-to-idle state transition", async () => {
    apiMocks.listRecordings.mockResolvedValueOnce([]).mockResolvedValueOnce([recordingFile]);
    render(<App />);
    await waitFor(() => expect(apiMocks.listRecordings).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(stateListener).toBeTypeOf("function"));

    act(() => stateListener?.({ ...recordingState, revision: 10 }));
    expect(apiMocks.listRecordings).toHaveBeenCalledTimes(1);

    act(() =>
      stateListener?.({
        ...stoppedState,
        message: "Recording stopped",
        revision: 11
      })
    );

    expect(await screen.findByRole("button", { name: "选择 demo-auto" })).toBeEnabled();
    expect(apiMocks.listRecordings).toHaveBeenCalledTimes(2);
  });

  it("serializes concurrent refreshes and follows up when data changes in flight", async () => {
    let resolveRefresh: ((recordings: (typeof recordingFile)[]) => void) | undefined;
    const refresh = new Promise<(typeof recordingFile)[]>((resolve) => {
      resolveRefresh = resolve;
    });
    apiMocks.listRecordings
      .mockResolvedValueOnce([])
      .mockReturnValueOnce(refresh)
      .mockResolvedValueOnce([recordingFile]);
    render(<App />);
    await waitFor(() => expect(apiMocks.listRecordings).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(stateListener).toBeTypeOf("function"));
    await waitFor(() => expect(recordingsChangedListener).toBeTypeOf("function"));

    act(() => {
      stateListener?.({ ...recordingState, revision: 10 });
      stateListener?.({ ...stoppedState, revision: 11 });
      recordingsChangedListener?.();
    });

    expect(apiMocks.listRecordings).toHaveBeenCalledTimes(2);
    act(() => resolveRefresh?.([]));
    await waitFor(() => expect(apiMocks.listRecordings).toHaveBeenCalledTimes(3));
    expect(await screen.findByRole("button", { name: "选择 demo-auto" })).toBeEnabled();
    expect(apiMocks.listRecordings).toHaveBeenCalledTimes(3);
  });

  it("clears the selected recording highlight when a new recording starts", async () => {
    apiMocks.listRecordings.mockResolvedValue([recordingFile]);
    apiMocks.loadRecording.mockResolvedValue({ ...stoppedState, revision: 3 });
    const user = userEvent.setup();
    render(<App />);

    const item = await screen.findByRole("button", { name: "选择 demo-auto" });
    await user.click(item);
    expect(item).toHaveAttribute("aria-pressed", "true");

    act(() => stateListener?.({ ...recordingState, revision: 4 }));
    await waitFor(() => expect(item).toHaveAttribute("aria-pressed", "false"));
  });

  it("does not trigger app hotkeys from form controls or IME composition", async () => {
    apiMocks.getHotkeys.mockResolvedValue({ record: "1", playback: "F12", stop: "1" });
    const user = userEvent.setup();
    render(<App />);

    const loopCount = await screen.findByLabelText("循环次数");
    await user.clear(loopCount);
    await user.type(loopCount, "1");
    fireEvent.keyDown(window, { key: "1", isComposing: true });

    expect(apiMocks.startRecording).not.toHaveBeenCalled();
    expect(loopCount).toHaveValue(1);
  });

  it("keeps a state subscription failure visible after an unrelated successful command", async () => {
    apiMocks.subscribeToState.mockRejectedValue(
      new Error("Command plugin:event|listen not allowed by ACL")
    );
    const user = userEvent.setup();
    render(<App />);

    const message = await screen.findByText(
      "没有权限监听应用状态变化，请重启应用后再试。"
    );
    await user.click(screen.getByRole("button", { name: "刷新录制文件" }));

    await waitFor(() => expect(apiMocks.listRecordings).toHaveBeenCalledTimes(2));
    expect(message).toBeInTheDocument();
  });

  it("shows asynchronous playback failures as localized alerts", async () => {
    apiMocks.getState.mockResolvedValue({
      ...stoppedState,
      message: "SendInput failed",
      message_is_error: true,
      revision: 6
    });
    render(<App />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "系统未能发送模拟输入，请检查应用权限。"
    );
  });

  it("localizes the playback stopping state", async () => {
    apiMocks.getState.mockResolvedValue({
      ...playingState,
      message: "Stopping playback",
      revision: 6
    });
    render(<App />);

    expect(await screen.findAllByText("正在停止回放")).toHaveLength(2);
  });

  it("requires confirmation before deleting and preserves a cancelled deletion", async () => {
    apiMocks.listRecordings.mockResolvedValue([recordingFile]);
    apiMocks.confirmDeleteRecording.mockResolvedValue(false);
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "删除 demo-auto" }));

    await waitFor(() =>
      expect(apiMocks.confirmDeleteRecording).toHaveBeenCalledWith(recordingFile.name)
    );
    expect(apiMocks.deleteRecording).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "选择 demo-auto" })).toBeInTheDocument();
  });

  it("keeps corrupt library entries visible but prevents loading them", async () => {
    apiMocks.listRecordings.mockResolvedValue([
      { ...recordingFile, load_error: "invalid recording json: unexpected end of file" }
    ]);
    render(<App />);

    expect(await screen.findByRole("button", { name: "无法载入 demo-auto" })).toBeDisabled();
    expect(screen.getByText("录制文件格式不正确。")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "删除 demo-auto" })).toBeEnabled();
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
