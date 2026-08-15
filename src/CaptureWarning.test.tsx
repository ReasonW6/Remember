import { act, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CaptureWarning } from "./CaptureWarning";

const eventMocks = vi.hoisted(() => ({
  emit: vi.fn(),
  listen: vi.fn()
}));

vi.mock("@tauri-apps/api/event", () => eventMocks);

describe("CaptureWarning", () => {
  interface WarningPayload {
    kind: "capture-unavailable" | "playback-waiting" | "playback-stopped";
    title: string;
    message: string;
    detail: string;
  }

  let warningListener: ((event: { payload: WarningPayload }) => void) | undefined;
  const unsubscribe = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    eventMocks.emit.mockResolvedValue(undefined);
    warningListener = undefined;
    eventMocks.listen.mockImplementation(
      async (_event: string, listener: (event: { payload: WarningPayload }) => void) => {
        warningListener = listener;
        return unsubscribe;
      }
    );
  });

  it("shows warning events and removes the listener on unmount", async () => {
    const { unmount } = render(<CaptureWarning />);
    await waitFor(() => expect(warningListener).toBeTypeOf("function"));
    await waitFor(() =>
      expect(eventMocks.emit).toHaveBeenCalledWith("remember://capture-warning-ready")
    );

    act(() => {
      warningListener?.({
        payload: {
          kind: "capture-unavailable",
          title: "此处无法录制",
          message: "该窗口无法读取，请使用管理员身份重试。",
          detail: "把鼠标移回可读取窗口后会自动继续。"
        }
      });
    });

    expect(screen.getByRole("status")).toHaveTextContent(
      "此处无法录制该窗口无法读取，请使用管理员身份重试。把鼠标移回可读取窗口后会自动继续。"
    );
    unmount();
    expect(unsubscribe).toHaveBeenCalledTimes(1);
  });

  it("renders playback failures as an assertive, visually distinct alert", async () => {
    render(<CaptureWarning />);
    await waitFor(() => expect(warningListener).toBeTypeOf("function"));

    act(() => {
      warningListener?.({
        payload: {
          kind: "playback-stopped",
          title: "回放已停止",
          message: "下拉框中没有名为“Mihomo”的选项。",
          detail: "准备好目标窗口或选项后，请重新开始回放。"
        }
      });
    });

    const alert = screen.getByRole("alert");
    expect(alert).toHaveClass("capture-warning-playback-stopped");
    expect(alert).toHaveTextContent("Mihomo");
  });

  it("contains a rejected event subscription without an unhandled rejection", async () => {
    eventMocks.listen.mockRejectedValue(new Error("event permission unavailable"));

    render(<CaptureWarning />);

    await waitFor(() => expect(eventMocks.listen).toHaveBeenCalledTimes(1));
    await act(async () => Promise.resolve());
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });
});
