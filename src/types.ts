export type AppMode = "idle" | "recording" | "playing";

export interface UiState {
  mode: AppMode;
  recording_name: string | null;
  step_count: number;
  duration_ms: number;
  message: string;
  revision: number;
  message_is_error: boolean;
}

export interface RecordingFile {
  name: string;
  path: string;
  step_count: number;
  duration_ms: number;
  created_at: string;
  updated_at_ms: number;
  load_error: string | null;
}

export interface HotkeyConfig {
  record: string;
  playback: string;
  stop: string;
}

export interface AdvancedSettingsConfig {
  feedback_volume_percent: number;
  feedback_muted: boolean;
  show_activity_indicator: boolean;
}

export interface SettingsBundle {
  advanced: AdvancedSettingsConfig;
  hotkeys: HotkeyConfig;
}

export interface PrivilegeState {
  is_elevated: boolean;
}
