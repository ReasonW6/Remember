export type AppMode = "idle" | "recording" | "playing";

export interface UiState {
  mode: AppMode;
  recording_name: string | null;
  step_count: number;
  duration_ms: number;
  message: string;
}
