import { act } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { Root } from "react-dom/client";
import activityHtml from "../activity-indicator.html?raw";
import advancedHtml from "../advanced-settings.html?raw";
import mainHtml from "../index.html?raw";
import activityEntry from "./activity-indicator-main.tsx?raw";
import advancedEntry from "./advanced-settings-main.tsx?raw";
import mainEntry from "./main.tsx?raw";
import { mountReactApp } from "./mountReactApp";

function parse(html: string) {
  return new DOMParser().parseFromString(html, "text/html");
}

describe("startup pages", () => {
  let mountedRoot: Root | undefined;

  afterEach(() => {
    if (mountedRoot) {
      act(() => mountedRoot?.unmount());
      mountedRoot = undefined;
    }
    document.body.replaceChildren();
  });

  it("ships a styled, inert compact shell before React starts", () => {
    const document = parse(mainHtml);
    const shell = document.querySelector("#startup-shell");

    expect(shell).not.toBeNull();
    expect(shell?.classList.contains("app-shell")).toBe(true);
    expect(shell?.classList.contains("compact-shell")).toBe(true);
    expect(shell?.textContent).toContain("Remember");
    expect(shell?.textContent).toContain("选择录制文件");
    expect(shell?.textContent).toContain("录制");
    expect(shell?.textContent).toContain("播放");
    expect(shell?.textContent).toContain("正在启动…");
    expect(shell?.querySelector("#compact-record-button")).toBeNull();
    expect(Array.from(shell?.querySelectorAll("button, select") ?? [])).not.toHaveLength(0);
    for (const control of shell?.querySelectorAll("button, select") ?? []) {
      expect(control.hasAttribute("disabled")).toBe(true);
    }
    expect(document.querySelector('link[rel="stylesheet"][href="/src/styles.css"]')).not.toBeNull();
  });

  it("uses one isolated static entry for each window", () => {
    const mainDocument = parse(mainHtml);
    const advancedDocument = parse(advancedHtml);
    const activityDocument = parse(activityHtml);

    expect(mainDocument.querySelector('script[src="/src/main.tsx"]')).not.toBeNull();
    expect(advancedDocument.querySelector('script[src="/src/advanced-settings-main.tsx"]')).not.toBeNull();
    expect(activityDocument.querySelector('script[src="/src/activity-indicator-main.tsx"]')).not.toBeNull();
    expect(advancedDocument.querySelector("#startup-shell")).toBeNull();
    expect(activityDocument.querySelector("#startup-shell")).toBeNull();
    expect(activityDocument.documentElement.classList.contains("activity-indicator-page")).toBe(
      true
    );
    expect(advancedDocument.querySelector('link[href="/src/styles.css"]')).not.toBeNull();
    expect(activityDocument.querySelector('link[href="/src/styles.css"]')).not.toBeNull();
  });

  it("statically imports only the component owned by each page", () => {
    expect(mainEntry).toContain('import { App } from "./App"');
    expect(mainEntry).not.toContain("import(");
    expect(advancedEntry).toContain(
      'import { AdvancedSettings } from "./AdvancedSettings"'
    );
    expect(activityEntry).toContain(
      'import { ActivityIndicator } from "./ActivityIndicator"'
    );
  });

  it("replaces the startup shell in the synchronous first React commit", () => {
    document.body.innerHTML = '<div id="root"><div id="startup-shell">正在启动…</div></div>';

    act(() => {
      mountedRoot = mountReactApp(
        <button id="compact-record-button" type="button">
          录制
        </button>
      );
    });

    expect(document.querySelector("#startup-shell")).toBeNull();
    expect(document.querySelector("#compact-record-button")).toHaveTextContent("录制");
  });
});
