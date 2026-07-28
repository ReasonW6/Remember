import { useRef } from "react";

interface PlaybackSettingsProps {
  loopCount: number | null;
  speedMultiplier: number;
  appliedLoopCount: number | null;
  appliedSpeedMultiplier: number;
  syncPending: boolean;
  syncReady: boolean;
  playbackHotkey: string;
  onLoopCountChange: (value: number | null) => void;
  onSpeedMultiplierChange: (value: number) => void;
}

function displayNumber(value: number) {
  return Number.isFinite(value) ? value : "";
}

const maxLoopCount = 0xffffffff;
const loopCountError = `循环次数必须是 1 到 ${maxLoopCount} 之间的整数。`;
const speedError = "速度必须是大于 0 的有效数字。";

export function PlaybackSettings({
  loopCount,
  speedMultiplier,
  appliedLoopCount,
  appliedSpeedMultiplier,
  syncPending,
  syncReady,
  playbackHotkey,
  onLoopCountChange,
  onSpeedMultiplierChange
}: PlaybackSettingsProps) {
  const finiteLoopCountRef = useRef(loopCount ?? 1);
  const isInfinite = loopCount === null;
  if (loopCount !== null) {
    finiteLoopCountRef.current = loopCount;
  }

  const loopValidationError =
    loopCount !== null &&
    (!Number.isInteger(loopCount) || loopCount < 1 || loopCount > maxLoopCount)
      ? loopCountError
      : "";
  const speedValidationError =
    !Number.isFinite(speedMultiplier) || speedMultiplier <= 0 ? speedError : "";
  const validationMessage = loopValidationError || speedValidationError;
  const appliedLoopDescription =
    appliedLoopCount === null ? "无限循环" : `循环 ${appliedLoopCount} 次`;
  const synchronizationMessage = validationMessage
    ? `当前输入无效；前台播放已禁用，全局 ${playbackHotkey} 仍使用已应用值。`
    : syncPending
      ? `正在应用新设置；前台播放已禁用，全局 ${playbackHotkey} 仍使用已应用值。`
      : !syncReady
        ? `新设置尚未应用；前台播放已禁用，全局 ${playbackHotkey} 仍使用已应用值。`
        : "";

  return (
    <section className="panel settings-panel" aria-labelledby="playback-settings-title">
      <h2 id="playback-settings-title">回放设置</h2>
      <fieldset className="loop-mode-fieldset">
        <legend>循环模式</legend>
        <label>
          <input
            type="radio"
            name="loop-mode"
            checked={!isInfinite}
            onChange={() => onLoopCountChange(finiteLoopCountRef.current)}
          />
          有限循环
        </label>
        <label>
          <input
            type="radio"
            name="loop-mode"
            checked={isInfinite}
            onChange={() => onLoopCountChange(null)}
          />
          无限循环
        </label>
      </fieldset>
      <div className="settings-grid">
        <label className="field">
          <span>循环次数</span>
          <input
            type="number"
            min="1"
            max={maxLoopCount}
            step="1"
            value={displayNumber(isInfinite ? finiteLoopCountRef.current : loopCount)}
            onChange={(event) => {
              finiteLoopCountRef.current = event.currentTarget.valueAsNumber;
              onLoopCountChange(event.currentTarget.valueAsNumber);
            }}
            disabled={isInfinite}
            aria-invalid={Boolean(loopValidationError)}
          />
        </label>
        <label className="field">
          <span>速度</span>
          <input
            type="number"
            min="0.1"
            step="0.1"
            value={displayNumber(speedMultiplier)}
            onChange={(event) => onSpeedMultiplierChange(event.currentTarget.valueAsNumber)}
            aria-invalid={Boolean(speedValidationError)}
          />
        </label>
      </div>
      {validationMessage ? (
        <p className="alert" role="alert">
          {validationMessage}
        </p>
      ) : null}
      <p
        className="playback-settings-status"
        role="status"
        aria-live="polite"
        aria-atomic="true"
      >
        已应用：{appliedLoopDescription}，速度 {appliedSpeedMultiplier} 倍。
        {synchronizationMessage ? ` ${synchronizationMessage}` : ""}
      </p>
    </section>
  );
}
