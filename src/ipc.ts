import { invoke } from "@tauri-apps/api/core";

/** Settings as sent to the backend for saving (includes raw API key). */
export interface Settings {
  api_key: string;
  model: string;
  media_resolution: string;
  concurrency: number;
  overwrite_policy: string;
  custom_prompt: string;
  split_threshold_gb: number;
  segment_duration_min: number;
  include_transcript: boolean;
}

/** Settings view returned from the backend (API key redacted). */
export interface SettingsView {
  api_key_set: boolean;
  api_key_preview: string;
  model: string;
  media_resolution: string;
  concurrency: number;
  overwrite_policy: string;
  custom_prompt: string;
  split_threshold_gb: number;
  segment_duration_min: number;
  include_transcript: boolean;
}

export interface ClipInfo {
  path: string;
  filename: string;
  file_size: number;
  status: string;
}

export interface ScanResult {
  folder_path: string;
  total_clips: number;
  queued: number;
  already_done: number;
  clips: ClipInfo[];
}

export interface FolderStats {
  total: number;
  done: number;
  errors: number;
  queued: number;
}

export interface ProgressEvent {
  type: "ClipDone" | "ClipError" | "ClipStatus" | "Complete";
  filename?: string;
  error?: string;
  status?: string;
}

export const ipc = {
  getSettings: () => invoke<SettingsView>("get_settings"),
  saveSettings: (s: Settings) => invoke<void>("save_settings", { newSettings: s }),
  scanFolder: (folderPath: string) => invoke<ScanResult>("scan_folder", { folderPath }),
  startProcessing: (folderPath: string) =>
    invoke<void>("start_processing", { folderPath }),
  getFolderStats: (folderPath: string) => invoke<FolderStats>("get_folder_stats", { folderPath }),
  regenerateIndex: (folderPath: string) =>
    invoke<string>("regenerate_index", { folderPath }),
  readIndexFile: (folderPath: string) => invoke<string>("read_index_file", { folderPath }),
};
