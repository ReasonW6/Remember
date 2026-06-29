import assert from "node:assert/strict";
import test from "node:test";
import { buildFlowOptions } from "./flowOptions.ts";
import type { Flow, FlowSummary, SavedFlow } from "./types.ts";

test("keeps an unsaved recording draft in the flow options after switching away", () => {
  const savedSummary: FlowSummary = {
    fileName: "daily-report.remember.json",
    name: "daily-report",
    displayName: "Daily Report",
    stepCount: 0,
    savedAt: 1,
    isValid: true,
    error: null,
  };
  const retainedDraft: SavedFlow = {
    fileName: "recording-123.remember.json",
    savedAt: 0,
    flow: draftFlow(),
  };

  const options = buildFlowOptions({
    flow: savedFlow(),
    flowSummaries: [savedSummary],
    selectedFileName: savedSummary.fileName,
    retainedDraft,
  });

  assert.deepEqual(
    options.map((option) => option.fileName),
    ["recording-123.remember.json", "daily-report.remember.json"],
  );
  assert.equal(options[0].displayName, "Recorded Draft");
  assert.equal(options[0].stepCount, 1);
});

function savedFlow(): Flow {
  return {
    version: 1,
    name: "daily-report",
    displayName: "Daily Report",
    targetWindow: {
      title: "Excel",
      process: "EXCEL.EXE",
      size: "800 x 600",
      matched: true,
    },
    steps: [],
  };
}

function draftFlow(): Flow {
  return {
    version: 1,
    name: "recording-123",
    displayName: "Recorded Draft",
    targetWindow: {
      title: "Notepad",
      process: "notepad.exe",
      size: "800 x 600",
      matched: true,
    },
    steps: [
      {
        type: "click",
        id: 1,
        action: "左键单击",
        target: "(10, 20) [屏幕绝对]",
        x: 10,
        y: 20,
        delayMs: 0,
        note: "",
      },
    ],
  };
}
