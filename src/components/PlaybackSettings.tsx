import { useRef } from "react";

interface PlaybackSettingsProps {
  loopCount: number | null;
  speedMultiplier: number;
  onLoopCountChange: (value: number | null) => void;
  onSpeedMultiplierChange: (value: number) => void;
}

function displayNumber(value: number) {
  return Number.isFinite(value) ? value : "";
}

const loopCountError = "循环次数必须是大于等于 1 的整数。";
const speedError = "速度必须是大于 0 的有效数字。";

export function PlaybackSettings({
  loopCount,
  speedMultiplier,
  onLoopCountChange,
  onSpeedMultiplierChange
}: PlaybackSettingsProps) {
  const finiteLoopCountRef = useRef(loopCount ?? 1);
  const isInfinite = loopCount === null;
  if (loopCount !== null) {
    finiteLoopCountRef.current = loopCount;
  }

  const loopValidationError =
    loopCount !== null && (!Number.isInteger(loopCount) || loopCount < 1)
      ? loopCountError
      : "";
  const speedValidationError =
    !Number.isFinite(speedMultiplier) || speedMultiplier <= 0 ? speedError : "";
  const validationMessage = loopValidationError || speedValidationError;

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
    </section>
  );
}
