import { invoke } from '@tauri-apps/api/core'

/** Mirrors `hanger_core::AppInfo`. */
export interface AppInfo {
  version: string
  claudeConfigDir: string | null
}

export function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>('app_info')
}
