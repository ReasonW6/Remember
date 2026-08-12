import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { WindowBinding } from "./WindowBinding";
import * as rememberApi from "./lib/rememberApi";
import type { WindowBindingRequest } from "./types";

vi.mock("./lib/rememberApi", () => ({
  subscribeToWindowBinding: vi.fn(),
  getPendingWindowBinding: vi.fn(),
  selectWindowBindingCandidate: vi.fn(),
  cancelWindowBinding: vi.fn(),
  highlightWindowBindingCandidate: vi.fn()
}));

const request: WindowBindingRequest = {
  request_id: 7,
  target: {
    executable_path: "C:\\Apps\\Editor.exe",
    window_class: "EditorWindow",
    title: "recorded.txt",
    client_size: { width: 800, height: 600 },
    dpi: 96
  },
  candidates: [
    {
      candidate_id: 0,
      executable_path: "C:\\Apps\\Editor.exe",
      window_class: "EditorWindow",
      title: "first.txt",
      client_size: { width: 800, height: 600 },
      dpi: 96,
      process_id: 101
    },
    {
      candidate_id: 1,
      executable_path: "C:\\Apps\\Editor.exe",
      window_class: "EditorWindow",
      title: "second.txt",
      client_size: { width: 800, height: 600 },
      dpi: 96,
      process_id: 102
    }
  ]
};

const newerRequest: WindowBindingRequest = {
  ...request,
  request_id: 8,
  target: { ...request.target, title: "new-recorded.txt" },
  candidates: request.candidates.map((candidate) => ({
    ...candidate,
    title: `new-${candidate.title}`
  }))
};

describe("WindowBinding", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(rememberApi.subscribeToWindowBinding).mockResolvedValue(vi.fn());
    vi.mocked(rememberApi.getPendingWindowBinding).mockResolvedValue(request);
    vi.mocked(rememberApi.selectWindowBindingCandidate).mockResolvedValue(undefined);
    vi.mocked(rememberApi.cancelWindowBinding).mockResolvedValue(undefined);
    vi.mocked(rememberApi.highlightWindowBindingCandidate).mockResolvedValue(undefined);
  });

  it("loads a pending request and selects the chosen candidate", async () => {
    const user = userEvent.setup();
    render(<WindowBinding />);

    const second = (await screen.findByText("second.txt")).closest("button");
    expect(second).not.toBeNull();
    expect(screen.getByRole("heading", { name: "需要确认目标窗口" })).toBeInTheDocument();
    expect(screen.getByText("recorded.txt")).toBeInTheDocument();
    expect(
      within(screen.getByRole("list", { name: "候选窗口" })).getAllByRole("button")
    ).toHaveLength(2);
    await user.click(second!);

    expect(rememberApi.selectWindowBindingCandidate).toHaveBeenCalledWith(7, 1);
  });

  it("highlights a candidate while it is hovered", async () => {
    const user = userEvent.setup();
    render(<WindowBinding />);

    const first = (await screen.findByText("first.txt")).closest("button");
    expect(first).not.toBeNull();
    await user.hover(first!);
    await waitFor(() =>
      expect(rememberApi.highlightWindowBindingCandidate).toHaveBeenLastCalledWith(7, 0)
    );
    await user.unhover(first!);
    await waitFor(() =>
      expect(rememberApi.highlightWindowBindingCandidate).toHaveBeenLastCalledWith(7, null)
    );
  });

  it("keeps a keyboard-focused candidate highlighted when the pointer leaves", async () => {
    const user = userEvent.setup();
    render(<WindowBinding />);

    const first = (await screen.findByText("first.txt")).closest("button");
    expect(first).not.toBeNull();
    first!.focus();
    await waitFor(() =>
      expect(rememberApi.highlightWindowBindingCandidate).toHaveBeenLastCalledWith(7, 0)
    );

    await user.hover(first!);
    await user.unhover(first!);
    await waitFor(() =>
      expect(rememberApi.highlightWindowBindingCandidate).toHaveBeenLastCalledWith(7, 0)
    );

    first!.blur();
    await waitFor(() =>
      expect(rememberApi.highlightWindowBindingCandidate).toHaveBeenLastCalledWith(7, null)
    );
  });

  it("cancels playback from the close button", async () => {
    const user = userEvent.setup();
    render(<WindowBinding />);

    await screen.findByText("first.txt");
    await user.click(screen.getByRole("button", { name: "取消回放" }));

    expect(rememberApi.cancelWindowBinding).toHaveBeenCalledWith(7);
  });

  it("accepts a request emitted after the window mounts", async () => {
    let onRequest: ((next: WindowBindingRequest) => void) | undefined;
    vi.mocked(rememberApi.subscribeToWindowBinding).mockImplementation(async (listener) => {
      onRequest = listener;
      return vi.fn();
    });
    vi.mocked(rememberApi.getPendingWindowBinding).mockResolvedValue(null);
    render(<WindowBinding />);

    await waitFor(() => expect(onRequest).toBeDefined());
    await act(async () => onRequest?.(request));

    expect(await screen.findByText("second.txt")).toBeInTheDocument();
  });

  it("does not let an older pending read overwrite a newer emitted request", async () => {
    let onRequest: ((next: WindowBindingRequest) => void) | undefined;
    let resolvePending: ((pending: WindowBindingRequest | null) => void) | undefined;
    vi.mocked(rememberApi.subscribeToWindowBinding).mockImplementation(async (listener) => {
      onRequest = listener;
      return vi.fn();
    });
    vi.mocked(rememberApi.getPendingWindowBinding).mockReturnValue(
      new Promise((resolve) => {
        resolvePending = resolve;
      })
    );
    render(<WindowBinding />);

    await waitFor(() => expect(rememberApi.getPendingWindowBinding).toHaveBeenCalled());
    act(() => onRequest?.(newerRequest));
    expect(await screen.findByText("new-second.txt")).toBeInTheDocument();

    await act(async () => {
      resolvePending?.(request);
    });
    expect(screen.queryByText("recorded.txt")).not.toBeInTheDocument();
    expect(screen.getByText("new-recorded.txt")).toBeInTheDocument();
  });

  it("unsubscribes when the component unmounts before listener setup completes", async () => {
    const unsubscribe = vi.fn();
    let resolveSubscription: ((unsubscribe: () => void) => void) | undefined;
    vi.mocked(rememberApi.subscribeToWindowBinding).mockReturnValue(
      new Promise((resolve) => {
        resolveSubscription = resolve;
      })
    );

    const { unmount } = render(<WindowBinding />);
    unmount();
    await act(async () => {
      resolveSubscription?.(unsubscribe);
    });

    expect(unsubscribe).toHaveBeenCalledOnce();
    expect(rememberApi.getPendingWindowBinding).not.toHaveBeenCalled();
  });

  it("clears an active highlight when the component unmounts", async () => {
    const unsubscribe = vi.fn();
    vi.mocked(rememberApi.subscribeToWindowBinding).mockResolvedValue(unsubscribe);
    const { unmount } = render(<WindowBinding />);

    await screen.findByText("first.txt");
    unmount();

    expect(unsubscribe).toHaveBeenCalledOnce();
    await waitFor(() =>
      expect(rememberApi.highlightWindowBindingCandidate).toHaveBeenLastCalledWith(7, null)
    );
  });
});
