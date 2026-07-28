import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { StatusPanel } from "./StatusPanel";

describe("StatusPanel", () => {
  it("does not repeat an error reported by both the command and state event", () => {
    const localizedError = "系统未能发送模拟输入，请检查应用权限。";
    render(
      <StatusPanel
        state={{
          mode: "idle",
          recording_name: "demo",
          step_count: 3,
          duration_ms: 1200,
          message: "SendInput failed",
          revision: 2,
          message_is_error: true
        }}
        error={`初始化失败。 ${localizedError}`}
      />
    );

    expect(screen.getByRole("alert")).toHaveTextContent(
      new RegExp(`^初始化失败。 ${localizedError}$`)
    );
  });
});
