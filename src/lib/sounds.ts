export type FeedbackTone =
  | "recording_start"
  | "recording_stop"
  | "playback_start"
  | "playback_stop";

const toneFrequency: Record<FeedbackTone, number> = {
  recording_start: 880,
  recording_stop: 440,
  playback_start: 660,
  playback_stop: 330
};

export function playFeedbackTone(
  tone: FeedbackTone,
  volumePercent = 50,
  muted = false
) {
  const normalizedVolume = Math.min(100, Math.max(0, volumePercent));
  if (muted || normalizedVolume === 0) {
    return;
  }

  const AudioContextClass = window.AudioContext ?? window.webkitAudioContext;
  if (!AudioContextClass) {
    return;
  }

  const context = new AudioContextClass();
  const oscillator = context.createOscillator();
  const gain = context.createGain();

  oscillator.type = "sine";
  oscillator.frequency.value = toneFrequency[tone];
  const peakGain = 0.48 * (normalizedVolume / 100);
  gain.gain.setValueAtTime(0.0001, context.currentTime);
  gain.gain.exponentialRampToValueAtTime(peakGain, context.currentTime + 0.01);
  gain.gain.exponentialRampToValueAtTime(0.0001, context.currentTime + 0.14);

  oscillator.connect(gain);
  gain.connect(context.destination);
  oscillator.start();
  oscillator.stop(context.currentTime + 0.15);
  oscillator.onended = () => {
    void context.close();
  };
}

declare global {
  interface Window {
    webkitAudioContext?: typeof AudioContext;
  }
}
