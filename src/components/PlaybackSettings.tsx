interface PlaybackSettingsProps {
  loopCount: number;
  speedMultiplier: number;
  onLoopCountChange: (value: number) => void;
  onSpeedMultiplierChange: (value: number) => void;
}

function inputNumber(value: number) {
  return Number.isNaN(value) ? 0 : value;
}

export function PlaybackSettings({
  loopCount,
  speedMultiplier,
  onLoopCountChange,
  onSpeedMultiplierChange
}: PlaybackSettingsProps) {
  const loopError = loopCount < 1 ? "Loop count must be at least 1." : "";
  const speedError = speedMultiplier <= 0 ? "Speed must be greater than 0." : "";
  const validationMessage = loopError || speedError;

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
            value={loopCount}
            onChange={(event) => onLoopCountChange(inputNumber(event.currentTarget.valueAsNumber))}
          />
        </label>
        <label className="field">
          <span>Speed</span>
          <input
            type="number"
            min="0.1"
            step="0.1"
            value={speedMultiplier}
            onChange={(event) =>
              onSpeedMultiplierChange(inputNumber(event.currentTarget.valueAsNumber))
            }
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
