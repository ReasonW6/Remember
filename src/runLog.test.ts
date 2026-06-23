import assert from "node:assert/strict";
import test from "node:test";
import {
  appendPlaybackFinishedLog,
  appendPlaybackStartLog,
  appendRunLog,
  findLatestSafetyStopLog,
  type RunLogEntry,
} from "./runLog.ts";

test("records playback lifecycle entries newest first with safety severity", () => {
  const started = appendPlaybackStartLog(
    [],
    {
      runId: 42,
      status: "playing",
      label: "回放中",
      flowName: "Safety Smoke",
      loopCount: 3,
      speedMultiplier: 2,
      message: "开始回放 Safety Smoke。",
    },
    1_000,
  );

  const stopped = appendPlaybackFinishedLog(
    started,
    {
      runId: 42,
      status: "stopped",
      label: "已停止",
      reason: "safetyStopped",
      flowName: "Safety Smoke",
      completedSteps: 1,
      skippedSteps: 1,
      loopCount: 3,
      message: "回放已安全停止；目标窗口不同。",
    },
    2_000,
  );

  assert.equal(stopped.length, 2);
  assert.equal(stopped[0].level, "danger");
  assert.equal(stopped[0].title, "安全停止");
  assert.equal(stopped[0].flowName, "Safety Smoke");
  assert.equal(stopped[0].runId, 42);
  assert.match(stopped[0].detail, /目标窗口不同/);
  assert.equal(stopped[1].level, "info");
  assert.equal(stopped[1].title, "开始回放");
  assert.equal(stopped[1].time, 1_000);
});

test("labels infinite loop playback explicitly in the run log", () => {
  const entries = appendPlaybackStartLog(
    [],
    {
      runId: 99,
      status: "playing",
      label: "回放中",
      flowName: "Loop Forever",
      loopCount: 0,
      speedMultiplier: 1,
      message: "开始回放 Loop Forever。",
    },
    1_000,
  );

  assert.match(entries[0].detail, /^无限循环 · 1x · /);
});

test("keeps only the newest run log entries", () => {
  let entries: RunLogEntry[] = [];
  for (let index = 1; index <= 10; index += 1) {
    entries = appendRunLog(
      entries,
      {
        id: `log-${index}`,
        time: index,
        level: "info",
        title: `Log ${index}`,
        detail: `Detail ${index}`,
      },
      4,
    );
  }

  assert.deepEqual(
    entries.map((entry) => entry.id),
    ["log-10", "log-9", "log-8", "log-7"],
  );
});

test("returns the latest current safety stop for a flow", () => {
  const safetyStop: RunLogEntry = {
    id: "safety",
    time: 2_000,
    level: "danger",
    title: "安全停止",
    detail: "目标窗口不同：录制时为 EXCEL.EXE，当前为 remember.exe。",
    flowName: "Daily Report",
    reason: "safetyStopped",
  };
  const completedLater: RunLogEntry = {
    id: "completed",
    time: 3_000,
    level: "success",
    title: "回放完成",
    detail: "回放完成；已执行 3 个步骤。",
    flowName: "Daily Report",
    reason: "completed",
  };

  assert.equal(findLatestSafetyStopLog([safetyStop], "Daily Report"), safetyStop);
  assert.equal(findLatestSafetyStopLog([completedLater, safetyStop], "Daily Report"), undefined);
  assert.equal(findLatestSafetyStopLog([safetyStop], "Other Flow"), undefined);
});
