import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { RecordingFile, UiState } from "../types";
import { AdministratorControl } from "./AdministratorControl";
import { CompactControls } from "./CompactControls";
import { Controls } from "./Controls";
import { RecordingList } from "./RecordingList";

const idleState: UiState = {
  mode: "idle",
  recording_name: null,
  step_count: 0,
  duration_ms: 0,
  message: "就绪",
  revision: 1,
  message_is_error: false
};

const recordings: RecordingFile[] = [
  {
    version: 1,
    name: "screen-only",
    path: "C:\\recordings\\screen-only.remember.json",
    step_count: 2,
    duration_ms: 120,
    created_at: "2026-08-09T00:00:00Z",
    updated_at_ms: 0,
    load_error: null
  },
  {
    version: 2,
    name: "window-relative",
    path: "C:\\recordings\\window-relative.remember.json",
    step_count: 4,
    duration_ms: 240,
    created_at: "2026-08-09T00:00:00Z",
    updated_at_ms: 0,
    load_error: null
  }
];

describe("recording coordinate controls", () => {
  it("places the expanded window-relative toggle beside the administrator control", async () => {
    const user = userEvent.setup();
    const onWindowRelativeChange = vi.fn();

    render(
      <>
        <Controls
          state={idleState}
          hasRecording={false}
          playbackValid
          pendingCommand={false}
          onRecord={vi.fn()}
          onPlay={vi.fn()}
          onStop={vi.fn()}
          onSave={vi.fn()}
          onOpen={vi.fn()}
          onAdvancedSettings={vi.fn()}
        />
        <AdministratorControl
          isElevated={false}
          disabled={false}
          windowRelativeEnabled={false}
          onWindowRelativeChange={onWindowRelativeChange}
          onRestart={vi.fn()}
        />
      </>
    );

    const primaryControls = screen.getByRole("region", { name: "控制" });
    expect(
      within(primaryControls).queryByRole("button", { name: "窗口相对录制" })
    ).not.toBeInTheDocument();

    const auxiliaryControls = screen.getByRole("region", {
      name: "窗口相对录制与管理员模式"
    });
    const toggle = within(auxiliaryControls).getByRole("button", {
      name: "窗口相对录制"
    });
    expect(
      within(auxiliaryControls).getByRole("button", { name: "以管理员身份重启" })
    ).toBeInTheDocument();
    expect(toggle).toHaveAttribute("aria-pressed", "false");
    expect(toggle).toHaveAccessibleDescription(
      "V2 录制文件会明文保存目标窗口的可执行文件完整路径、类名和标题，应视为敏感文件。"
    );
    expect(toggle).toHaveAttribute("title", expect.stringMatching(/明文.*完整路径.*类名.*标题.*敏感/));
    await user.click(toggle);
    expect(onWindowRelativeChange).toHaveBeenCalledWith(true);
  });

  it("keeps the enabled state visible while the toggle is locked during recording", () => {
    render(
      <AdministratorControl
        isElevated={false}
        disabled
        windowRelativeEnabled
        onWindowRelativeChange={vi.fn()}
        onRestart={vi.fn()}
      />
    );

    const toggle = screen.getByRole("button", { name: "窗口相对录制" });
    expect(toggle).toBeDisabled();
    expect(toggle).toHaveClass("enabled");
  });

  it("shows V1 and V2 in compact recording choices", () => {
    render(
      <CompactControls
        state={idleState}
        recordings={recordings}
        selectedPath={null}
        selectedName={null}
        hasRecording={false}
        playbackValid
        pendingCommand={false}
        isElevated={false}
        windowRelativeEnabled={false}
        message="就绪"
        error=""
        onSelect={vi.fn()}
        onWindowRelativeChange={vi.fn()}
        onRecord={vi.fn()}
        onPlay={vi.fn()}
        onStop={vi.fn()}
        onRestartAsAdministrator={vi.fn()}
      />
    );

    expect(screen.getByRole("option", { name: "screen-only [V1]" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "window-relative [V2]" })).toBeInTheDocument();

    const toggle = screen.getByRole("button", { name: "启用窗口相对录制" });
    expect(toggle).toHaveAccessibleDescription(
      "V2 录制文件会明文保存目标窗口的可执行文件完整路径、类名和标题，应视为敏感文件。"
    );
    expect(toggle).toHaveAttribute("title", expect.stringMatching(/明文.*完整路径.*类名.*标题.*敏感/));
  });
});

describe("RecordingList", () => {
  it("adds only compact version badges to recording entries", () => {
    render(
      <RecordingList
        recordings={recordings}
        selectedPath={null}
        disabled={false}
        onSelect={vi.fn()}
        onDelete={vi.fn()}
        onRename={vi.fn()}
        onRefresh={vi.fn()}
      />
    );

    expect(screen.getByText("V1", { selector: ".recording-version" })).toBeInTheDocument();
    expect(screen.getByText("V2", { selector: ".recording-version" })).toBeInTheDocument();
    expect(screen.queryByText(/个目标|窗口相对/)).not.toBeInTheDocument();
  });
});
