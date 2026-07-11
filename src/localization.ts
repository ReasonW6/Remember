import type { AppMode } from "./types";

const modeLabels: Record<AppMode, string> = {
  idle: "就绪",
  recording: "正在录制",
  playing: "正在回放"
};

const messageLabels: Record<string, string> = {
  Idle: "就绪",
  Recording: "正在录制",
  "Recording stopped": "录制已停止",
  "Recording loaded": "录制已载入",
  Playing: "正在回放",
  "Playback stopped": "回放已停止",
  "Playback finished": "回放完成",
  Ready: "就绪",
  "Loaded recording": "录制已载入"
};

const errorLabels: Array<[string, string]> = [
  ["Command plugin:dialog|open not allowed by ACL", "没有权限打开文件选择窗口，请重启应用后再试。"],
  ["Command plugin:dialog|save not allowed by ACL", "没有权限打开保存窗口，请重启应用后再试。"],
  ["Command plugin:event|listen not allowed by ACL", "没有权限监听应用状态变化，请重启应用后再试。"],
  ["HotKey already registered", "这个快捷键已被其他程序占用，请换一个键。"],
  ["playback hotkey must be different", "播放快捷键不能和录制或停止相同。"],
  ["unsupported hotkey key", "不支持这个快捷键，请换一个键。"],
  ["hotkey key cannot be only a modifier", "快捷键不能只设置为 Ctrl、Alt、Shift 或 Win。"],
  ["hotkey key cannot be empty", "快捷键不能为空。"],
  ["Couldn't identify any key", "快捷键格式不正确。"],
  ["no recording loaded", "还没有可回放的录制文件。"],
  ["cannot play while recording", "录制中不能回放。"],
  ["cannot play while playing", "正在回放中。"],
  ["cannot record while recording", "正在录制中。"],
  ["cannot record while playing", "回放中不能开始录制。"],
  ["cannot load recording while recording", "录制中不能载入录制文件。"],
  ["cannot load recording while playing", "回放中不能载入录制文件。"],
  ["current recording has not been saved", "当前录制尚未保存，不能开始或载入其他录制。"],
  ["recording name cannot be empty", "录制名称不能为空。"],
  ["SendInput failed", "系统未能发送模拟输入，请检查应用权限。"],
  ["invalid recording json", "录制文件格式不正确。"],
  ["unsupported recording version", "录制文件版本不受支持。"],
  ["recording path is outside the library", "只能删除录制文件列表中的文件。"],
  ["file error", "文件读写失败，请检查路径或权限。"],
  ["state lock poisoned", "应用状态暂时不可用，请重启应用后再试。"]
];

export function displayMode(mode: AppMode) {
  return modeLabels[mode];
}

export function displayMessage(message: string) {
  return messageLabels[message] ?? message;
}

export function displayErrorMessage(error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  const match = errorLabels.find(([source]) => message.includes(source));
  return match ? match[1] : message;
}
