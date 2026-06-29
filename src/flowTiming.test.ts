import assert from "node:assert/strict";
import test from "node:test";
import { parseSpeedMultiplier } from "./flowTiming.ts";

test("parses playback speed labels with a safe fallback", () => {
  assert.equal(parseSpeedMultiplier("0.5x"), 0.5);
  assert.equal(parseSpeedMultiplier("2x"), 2);
  assert.equal(parseSpeedMultiplier("bad"), 1);
  assert.equal(parseSpeedMultiplier("0x"), 1);
});
