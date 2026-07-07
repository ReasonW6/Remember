import { afterEach, describe, expect, it, vi } from "vitest";
import { playFeedbackTone } from "./sounds";

describe("playFeedbackTone", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    // @ts-expect-error test cleanup for jsdom AudioContext replacement
    delete window.AudioContext;
  });

  it("uses an audible peak gain for feedback tones", () => {
    const ramp = vi.fn();
    const gainNode = {
      gain: {
        setValueAtTime: vi.fn(),
        exponentialRampToValueAtTime: ramp
      },
      connect: vi.fn()
    };
    const oscillator = {
      type: "",
      frequency: { value: 0 },
      connect: vi.fn(),
      start: vi.fn(),
      stop: vi.fn(),
      onended: undefined as (() => void) | undefined
    };
    const context = {
      currentTime: 10,
      destination: {},
      createOscillator: vi.fn(() => oscillator),
      createGain: vi.fn(() => gainNode),
      close: vi.fn()
    };
    const AudioContextMock = vi.fn(() => context);
    // @ts-expect-error partial AudioContext mock for unit test
    window.AudioContext = AudioContextMock;

    playFeedbackTone("recording_start");

    expect(ramp).toHaveBeenCalledWith(0.24, 10.01);
  });
});
