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
  version: number | null;
  name: string;
  path: string;
  step_count: number;
  duration_ms: number;
  created_at: string;
  updated_at_ms: number;
  load_error: string | null;
}

export interface ClientSize {
  width: number;
  height: number;
}

export interface WindowBindingTarget {
  executable_path: string;
  window_class: string;
  title: string;
  client_size: ClientSize;
  dpi: number;
}

export interface WindowBindingCandidate extends WindowBindingTarget {
  candidate_id: number;
  process_id: number;
}

export interface WindowBindingRequest {
  request_id: number;
  target: WindowBindingTarget;
  candidates: WindowBindingCandidate[];
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
  window_relative_recording_enabled: boolean;
}

export interface MainWindowPreferences {
  compact: boolean;
  position: { x: number; y: number } | null;
}

export interface SettingsBundle {
  advanced: AdvancedSettingsConfig;
  hotkeys: HotkeyConfig;
  main_window: MainWindowPreferences;
}

export interface PrivilegeState {
  is_elevated: boolean;
}
