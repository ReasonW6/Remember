import assert from "node:assert/strict";
import test from "node:test";
import {
  deleteStep,
  insertWaitStepAfter,
  selectExistingStepId,
  updateStepClickCoordinates,
  updateStepHotkeyText,
  updateStepDelayMs,
  updateStepText,
  updateTargetWindowMatched,
} from "./flowEditing.ts";
import type { Flow } from "./types.ts";

test("selects the current step while it exists and falls back to the first step", () => {
  const flow = editableFlow();

  assert.equal(selectExistingStepId(flow, 2), 2);
  assert.equal(selectExistingStepId(flow, 99), 1);
  assert.equal(selectExistingStepId({ ...flow, steps: [] }, 2), null);
});

test("updates a normal step delay without mutating the original flow", () => {
  const flow = editableFlow();

  const edited = updateStepDelayMs(flow, 1, 450);

  assert.equal(edited.steps[0].delayMs, 450);
  assert.equal(flow.steps[0].delayMs, 200);
  assert.equal(edited.steps[1].delayMs, 800);
  assert.notEqual(edited, flow);
});

test("updates wait duration and delay together because playback uses durationMs", () => {
  const flow = editableFlow();

  const edited = updateStepDelayMs(flow, 2, 1200);
  const waitStep = edited.steps[1];

  assert.equal(waitStep.type, "wait");
  if (waitStep.type !== "wait") throw new Error("expected wait step");
  assert.equal(waitStep.delayMs, 1200);
  assert.equal(waitStep.durationMs, 1200);
});

test("clamps invalid delay edits to zero milliseconds", () => {
  const edited = updateStepDelayMs(editableFlow(), 1, -50);

  assert.equal(edited.steps[0].delayMs, 0);
});

test("updates click coordinates and target label without mutating the original flow", () => {
  const flow = editableFlow();

  const edited = updateStepClickCoordinates(flow, 1, 320.4, 480.6);
  const clickStep = edited.steps[0];

  assert.equal(clickStep.type, "click");
  if (clickStep.type !== "click") throw new Error("expected click step");
  assert.equal(clickStep.x, 320);
  assert.equal(clickStep.y, 481);
  assert.equal(clickStep.target, "(320, 481) [屏幕绝对]");
  assert.equal(flow.steps[0].type, "click");
  if (flow.steps[0].type !== "click") throw new Error("expected original click step");
  assert.equal(flow.steps[0].x, 10);
  assert.equal(flow.steps[0].y, 20);
});

test("updates a type step text without mutating the original flow", () => {
  const flow = editableFlow();

  const edited = updateStepText(flow, 3, "Updated report title");
  const typeStep = edited.steps[2];

  assert.equal(typeStep.type, "type");
  if (typeStep.type !== "type") throw new Error("expected type step");
  assert.equal(typeStep.text, "Updated report title");
  assert.equal(flow.steps[2].type, "type");
  if (flow.steps[2].type !== "type") throw new Error("expected original type step");
  assert.equal(flow.steps[2].text, "Daily Report");
});

test("updates a hotkey step from editable text", () => {
  const edited = updateStepHotkeyText(editableFlow(), 4, "Ctrl + Shift + S");
  const hotkeyStep = edited.steps[3];

  assert.equal(hotkeyStep.type, "hotkey");
  if (hotkeyStep.type !== "hotkey") throw new Error("expected hotkey step");
  assert.deepEqual(hotkeyStep.keys, ["Ctrl", "Shift", "S"]);
});

test("updates the target-window matching requirement without mutating the original flow", () => {
  const flow = editableFlow();

  const edited = updateTargetWindowMatched(flow, false);

  assert.equal(edited.targetWindow.matched, false);
  assert.equal(flow.targetWindow.matched, true);
  assert.notEqual(edited, flow);
  assert.notEqual(edited.targetWindow, flow.targetWindow);
});

test("inserts a wait step after the selected step without mutating the original flow", () => {
  const flow = editableFlow();

  const result = insertWaitStepAfter(flow, 1, 750);
  const insertedStep = result.flow.steps[1];

  assert.equal(result.flow.steps.length, 5);
  assert.equal(result.flow.steps[0].id, 1);
  assert.equal(result.flow.steps[2].id, 2);
  assert.equal(insertedStep.type, "wait");
  if (insertedStep.type !== "wait") throw new Error("expected wait step");
  assert.equal(insertedStep.id, 5);
  assert.equal(insertedStep.durationMs, 750);
  assert.equal(insertedStep.delayMs, 750);
  assert.equal(insertedStep.note, "插入等待");
  assert.equal(result.selectedStepId, 5);
  assert.equal(flow.steps.length, 4);
});

test("deletes a step and selects the next available step", () => {
  const result = deleteStep(editableFlow(), 1);

  assert.equal(result.flow.steps.length, 3);
  assert.equal(result.flow.steps[0].id, 2);
  assert.equal(result.selectedStepId, 2);
});

test("deletes the last step and selects the previous step", () => {
  const result = deleteStep(editableFlow(), 4);

  assert.equal(result.flow.steps.length, 3);
  assert.equal(result.flow.steps[2].id, 3);
  assert.equal(result.selectedStepId, 3);
});

test("returns the original flow when deleting a missing step", () => {
  const flow = editableFlow();
  const result = deleteStep(flow, 99);

  assert.equal(result.flow, flow);
  assert.equal(result.selectedStepId, 1);
});

function editableFlow(): Flow {
  return {
    version: 1,
    name: "editable",
    displayName: "Editable",
    targetWindow: {
      title: "Notepad",
      process: "notepad.exe",
      size: "800 x 600",
      matched: true,
    },
    steps: [
      {
        id: 1,
        type: "click",
        action: "左键单击",
        target: "(10, 20) [屏幕绝对]",
        x: 10,
        y: 20,
        delayMs: 200,
        note: "click",
      },
      {
        id: 2,
        type: "wait",
        action: "等待",
        durationMs: 800,
        delayMs: 800,
        note: "wait",
      },
      {
        id: 3,
        type: "type",
        action: "文本输入",
        text: "Daily Report",
        delayMs: 300,
        note: "type",
      },
      {
        id: 4,
        type: "hotkey",
        action: "快捷键",
        keys: ["Ctrl", "S"],
        delayMs: 100,
        note: "hotkey",
      },
    ],
  };
}
