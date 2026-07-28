import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AdvancedSettings } from "./AdvancedSettings";

const apiMocks = vi.hoisted(() => ({
  getSettingsBundle: vi.fn(),
  setSettingsBundle: vi.fn()
}));

const windowMocks = vi.hoisted(() => ({
  startDragging: vi.fn(),
  minimize: vi.fn(),
  close: vi.fn()
}));

const soundMocks = vi.hoisted(() => ({
  playFeedbackTone: vi.fn()
}));

vi.mock("./lib/rememberApi", () => apiMocks);
vi.mock("./lib/sounds", () => soundMocks);
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => windowMocks
}));

const settings = {
  feedback_volume_percent: 50,
  feedback_muted: false,
  show_activity_indicator: true
};

const hotkeys = {
  record: "F8",
  playback: "F12",
  stop: "F8"
};

async function waitForSettingsLoaded() {
  await waitFor(() => expect(screen.getByRole("button", { name: "保存" })).toBeEnabled());
}

describe("AdvancedSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    apiMocks.getSettingsBundle.mockResolvedValue({ advanced: settings, hotkeys });
    apiMocks.setSettingsBundle.mockImplementation(async (bundle) => bundle);
    windowMocks.startDragging.mockResolvedValue(undefined);
    windowMocks.minimize.mockResolvedValue(undefined);
    windowMocks.close.mockResolvedValue(undefined);
  });

  it("loads sound, hotkey, and indicator settings in a separate window", async () => {
    render(<AdvancedSettings />);
    await waitForSettingsLoaded();

    expect(screen.getByRole("heading", { name: "高级设置", level: 1 })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "最小化" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "关闭" })).toBeInTheDocument();
    expect(await screen.findByRole("slider", { name: "提示音音量" })).toHaveValue("50");
    expect(screen.getByRole("button", { name: "静音" })).toHaveAttribute(
      "aria-pressed",
      "false"
    );
    expect(screen.getByLabelText(/显示左上角操作提示/)).toBeChecked();
    expect(screen.getByText("显示左上角操作提示")).toBeInTheDocument();
    expect(screen.queryByText(/录制绿点/)).not.toBeInTheDocument();
    expect(
      screen.queryByText("50% 与当前版本声音大小一致，0% 为静音。")
    ).not.toBeInTheDocument();
    expect(screen.getByText("快捷键")).toBeInTheDocument();
    expect(screen.getAllByText("F8", { selector: "kbd" })).toHaveLength(2);
    expect(screen.getAllByRole("button", { name: "保存" })).toHaveLength(1);
    expect(screen.queryByRole("button", { name: "保存快捷键" })).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "保存声音和提示设置" })
    ).not.toBeInTheDocument();
  });

  it("previews the final selected volume only after adjustment ends", async () => {
    render(<AdvancedSettings />);
    await waitForSettingsLoaded();
    const volume = await screen.findByRole("slider", { name: "提示音音量" });

    fireEvent.change(volume, { target: { value: "30" } });
    fireEvent.change(volume, { target: { value: "75" } });
    expect(soundMocks.playFeedbackTone).not.toHaveBeenCalled();

    fireEvent.pointerUp(volume);
    expect(soundMocks.playFeedbackTone).toHaveBeenCalledOnce();
    expect(soundMocks.playFeedbackTone).toHaveBeenLastCalledWith(
      "recording_start",
      75,
      false
    );

    fireEvent.change(volume, { target: { value: "80" } });
    fireEvent.keyUp(volume, { key: "ArrowRight" });
    expect(soundMocks.playFeedbackTone).toHaveBeenCalledTimes(2);
    expect(soundMocks.playFeedbackTone).toHaveBeenLastCalledWith(
      "recording_start",
      80,
      false
    );
  });

  it("disables and dims the volume slider while muted", async () => {
    const user = userEvent.setup();
    render(<AdvancedSettings />);
    await waitForSettingsLoaded();

    const volume = await screen.findByRole("slider", { name: "提示音音量" });
    await user.click(screen.getByRole("button", { name: "静音" }));

    expect(volume).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "恢复声音" }));
    expect(volume).toBeEnabled();
  });

  it("saves sound, indicator, and hotkey settings with the single save button", async () => {
    const nextHotkeys = {
      record: "Ctrl+Shift+R",
      playback: "F12",
      stop: "Ctrl+Shift+R"
    };
    const user = userEvent.setup();
    render(<AdvancedSettings />);
    await waitForSettingsLoaded();

    const volume = await screen.findByRole("slider", { name: "提示音音量" });
    fireEvent.change(volume, { target: { value: "75" } });
    await user.click(screen.getByRole("button", { name: "静音" }));
    await user.click(screen.getByLabelText(/显示左上角操作提示/));
    const recordHotkey = screen.getByLabelText("录制快捷键");
    await user.click(recordHotkey);
    await user.keyboard("{Control>}{Shift>}r{/Shift}{/Control}");
    const stopHotkey = screen.getByLabelText("停止快捷键");
    await user.click(stopHotkey);
    await user.keyboard("{Control>}{Shift>}r{/Shift}{/Control}");

    expect(apiMocks.setSettingsBundle).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(() =>
      expect(apiMocks.setSettingsBundle).toHaveBeenCalledWith({
        advanced: {
          feedback_volume_percent: 75,
          feedback_muted: true,
          show_activity_indicator: false
        },
        hotkeys: nextHotkeys
      })
    );
    expect(await screen.findByText("已保存")).toBeInTheDocument();
  });

  it("automatically hides the saved notice", async () => {
    render(<AdvancedSettings />);
    await waitForSettingsLoaded();
    const save = await screen.findByRole("button", { name: "保存" });
    vi.useFakeTimers();

    await act(async () => {
      fireEvent.click(save);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(screen.getByText("已保存")).toBeInTheDocument();

    act(() => vi.advanceTimersByTime(1799));
    expect(screen.getByText("已保存")).toBeInTheDocument();
    act(() => vi.advanceTimersByTime(1));
    expect(screen.queryByText("已保存")).not.toBeInTheDocument();
    vi.useRealTimers();
  });

  it("rejects unsafe single-key shortcuts and allows capture cancellation", async () => {
    const user = userEvent.setup();
    render(<AdvancedSettings />);
    await waitForSettingsLoaded();

    const capture = await screen.findByLabelText("录制快捷键");
    await user.click(capture);
    await user.keyboard("a");

    expect(screen.getByRole("alert")).toHaveTextContent(
      "仅支持字母、数字、F1-F24 及常用导航键；F1-F24 以外的按键必须搭配修饰键。"
    );
    await user.click(screen.getByRole("button", { name: "取消快捷键捕获" }));
    expect(capture).toHaveTextContent("F8");
    expect(capture).toHaveAttribute("aria-pressed", "false");
  });

  it("does not capture a key until capture mode starts", async () => {
    render(<AdvancedSettings />);
    await waitForSettingsLoaded();

    const capture = await screen.findByLabelText("录制快捷键");
    act(() => {
      capture.focus();
    });
    fireEvent.keyDown(capture, { key: "A" });

    expect(capture).toHaveTextContent("F8");
  });

  it("reports an atomic bundle save failure without issuing compensation writes", async () => {
    apiMocks.setSettingsBundle.mockRejectedValue(new Error("HotKey already registered"));
    const user = userEvent.setup();
    render(<AdvancedSettings />);
    await waitForSettingsLoaded();

    const volume = await screen.findByRole("slider", { name: "提示音音量" });
    fireEvent.change(volume, { target: { value: "75" } });
    await user.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(() => expect(apiMocks.setSettingsBundle).toHaveBeenCalledTimes(1));
    expect(apiMocks.setSettingsBundle).toHaveBeenCalledWith({
      advanced: {
        ...settings,
        feedback_volume_percent: 75
      },
      hotkeys
    });
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "这个快捷键已被其他程序占用，请换一个键。"
    );
    expect(volume).toHaveValue("75");
    expect(screen.queryByText("已保存")).not.toBeInTheDocument();
  });

  it("keeps defaults locked when the settings bundle cannot be loaded", async () => {
    apiMocks.getSettingsBundle.mockRejectedValue(new Error("settings unavailable"));
    const user = userEvent.setup();
    render(<AdvancedSettings />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "设置加载失败，保存已禁用。settings unavailable"
    );
    expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
    expect(screen.getByRole("slider", { name: "提示音音量" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "保存" }));
    expect(apiMocks.setSettingsBundle).not.toHaveBeenCalled();
  });
});
