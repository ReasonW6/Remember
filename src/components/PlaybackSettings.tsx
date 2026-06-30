interface PlaybackSettingsProps {
  loopCount: number;
  speedMultiplier: number;
  onLoopCountChange: (value: number) => void;
  onSpeedMultiplierChange: (value: number) => void;
}

function displayNumber(value: number) {
  return Number.isFinite(value) ? value : "";
}

const loopCountError = "Loop count must be a whole number of 1 or more.";
const speedError = "Speed must be a finite number greater than 0.";

export function PlaybackSettings({
  loopCount,
  speedMultiplier,
  onLoopCountChange,
  onSpeedMultiplierChange
}: PlaybackSettingsProps) {
  const loopValidationError =
    !Number.isSafeInteger(loopCount) || loopCount < 1 ? loopCountError : "";
  const speedValidationError =
    !Number.isFinite(speedMultiplier) || speedMultiplier <= 0 ? speedError : "";
  const validationMessage = loopValidationError || speedValidationError;

  return (
    <section className="panel settings-panel" aria-labelledby="playback-settings-title">
      <h2 id="playback-settings-title">Playback settings</h2>
      <div className="settings-grid">
        <label className="field">
          <span>Loop count</span>
          <input
            type="number"
            min="1"
            step="1"
            value={displayNumber(loopCount)}
            onChange={(event) => onLoopCountChange(event.currentTarget.valueAsNumber)}
          />
        </label>
        <label className="field">
          <span>Speed</span>
          <input
            type="number"
            min="0.1"
            step="0.1"
            value={displayNumber(speedMultiplier)}
            onChange={(event) => onSpeedMultiplierChange(event.currentTarget.valueAsNumber)}
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
