import type { Flow, SavedFlow } from "../types";

export const sampleFlow: Flow = {
  version: 1,
  name: "daily-report",
  displayName: "Daily Report 自动化",
  targetWindow: {
    title: "Sales Report - Excel",
    process: "EXCEL.EXE",
    size: "1920 x 1080",
    matched: true,
  },
  steps: [
    {
      id: 1,
      type: "click",
      action: "左键单击",
      target: "(120, 240) [屏幕绝对]",
      x: 120,
      y: 240,
      delayMs: 200,
      note: "打开菜单",
    },
    {
      id: 2,
      type: "type",
      action: "文本输入",
      text: "Daily Report",
      delayMs: 300,
      note: "输入标题",
    },
    {
      id: 3,
      type: "wait",
      action: "等待",
      durationMs: 2000,
      delayMs: 2000,
      note: "等待页面加载",
    },
    {
      id: 4,
      type: "click",
      action: "左键单击",
      target: "(540, 320) [导出按钮]",
      x: 540,
      y: 320,
      delayMs: 200,
      note: "点击导出",
    },
    {
      id: 5,
      type: "type",
      action: "文本输入",
      text: "=TODAY(yyyy-mm-dd)",
      delayMs: 300,
      note: "文件名",
    },
    {
      id: 6,
      type: "wait",
      action: "等待",
      durationMs: 1000,
      delayMs: 1000,
      note: "等待保存完成",
    },
    {
      id: 7,
      type: "hotkey",
      action: "快捷键",
      keys: ["Ctrl", "S"],
      delayMs: 100,
      note: "保存文件",
    },
    {
      id: 8,
      type: "wait",
      action: "等待",
      durationMs: 500,
      delayMs: 500,
      note: "短暂等待",
    },
  ],
};

export const sampleSavedFlow: SavedFlow = {
  fileName: "daily-report.remember.json",
  savedAt: 0,
  flow: sampleFlow,
};
