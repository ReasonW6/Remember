import { describe, expect, it } from "vitest";
import { isAllowedGlobalShortcut, shortcutFromEvent } from "./hotkeys";

describe("hotkeys", () => {
  it("allows only function keys without modifiers", () => {
    expect(isAllowedGlobalShortcut("F8")).toBe(true);
    expect(isAllowedGlobalShortcut("F24")).toBe(true);
    expect(isAllowedGlobalShortcut("A")).toBe(false);
    expect(isAllowedGlobalShortcut("Tab")).toBe(false);
    expect(isAllowedGlobalShortcut("Esc")).toBe(false);
  });

  it("allows character and navigation keys with modifiers", () => {
    expect(isAllowedGlobalShortcut("Ctrl+A")).toBe(true);
    expect(isAllowedGlobalShortcut("Ctrl+1")).toBe(true);
    expect(isAllowedGlobalShortcut("Ctrl+Space")).toBe(true);
    expect(isAllowedGlobalShortcut("Shift+Tab")).toBe(true);
    expect(isAllowedGlobalShortcut("Ctrl+Esc")).toBe(true);
    expect(isAllowedGlobalShortcut("Ctrl+;")).toBe(false);
    expect(isAllowedGlobalShortcut("Ctrl+Numpad1")).toBe(false);
  });

  it("keeps modifier information when capturing Escape", () => {
    expect(
      shortcutFromEvent({
        key: "Escape",
        ctrlKey: true,
        altKey: false,
        shiftKey: false,
        metaKey: false
      })
    ).toBe("Ctrl+Esc");
  });

  it("canonicalizes Space and shifted digits to Rust-supported keys", () => {
    expect(
      shortcutFromEvent({
        key: " ",
        code: "Space",
        ctrlKey: true,
        altKey: false,
        shiftKey: false,
        metaKey: false
      })
    ).toBe("Ctrl+Space");
    expect(
      shortcutFromEvent({
        key: "!",
        code: "Digit1",
        ctrlKey: false,
        altKey: false,
        shiftKey: true,
        metaKey: false
      })
    ).toBe("Shift+1");
  });
});
