import { act, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CaptureWarning } from "./CaptureWarning";

const eventMocks = vi.hoisted(() => ({
  listen: vi.fn()
}));

vi.mock("@tauri-apps/api/event", () => eventMocks);

describe("CaptureWarning", () => {
  let warningListener: ((event: { payload: { message: string } }) => void) | undefined;
  const unsubscribe = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    warningListener = undefined;
    eventMocks.listen.mockImplementation(
      async (_event: string, listener: (event: { payload: { message: string } }) => void) => {
        warningListener = listener;
        return unsubscribe;
      }
    );
  });

  it("shows warning events and removes the listener on unmount", async () => {
    const { unmount } = render(<CaptureWarning />);
    await waitFor(() => expect(warningListener).toBeTypeOf("function"));

    act(() => {
      warningListener?.({ payload: { message: "该窗口无法读取，请使用管理员身份重试。" } });
    });

    expect(screen.getByRole("status")).toHaveTextContent(
      "该窗口无法读取，请使用管理员身份重试。"
    );
    unmount();
    expect(unsubscribe).toHaveBeenCalledTimes(1);
  });

  it("contains a rejected event subscription without an unhandled rejection", async () => {
    eventMocks.listen.mockRejectedValue(new Error("event permission unavailable"));

    render(<CaptureWarning />);

    await waitFor(() => expect(eventMocks.listen).toHaveBeenCalledTimes(1));
    await act(async () => Promise.resolve());
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });
});
