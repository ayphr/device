export type CurrentPage = 'home' | 'stats' | 'settings' | 'profile' | 'device' | 'setup' | 'auth';

/** Payload returned by the connect/authenticate/setup Tauri commands. */
export interface DeviceConnectionState {
  connected: boolean;
  authenticated: boolean;
  authRequired: boolean;
  wifiRequired: boolean;
  setupComplete: boolean;
  deviceName: string;
}

/** Payload returned by the get_firmware_info_* Tauri commands. */
export interface FirmwareInfoResult {
  version: string;
  hardwareRev: string;
  uptimeSecs: number;
}

/** Payload of the `firmware-update-progress` event. */
export interface FirmwareUpdateProgress {
  step: string;
  progress: number;
  message: string;
}

/** `version.json` published alongside each firmware release. */
export interface FirmwareReleaseMetadata {
  version?: string;
  binaryUrl?: string;
  sha256?: string;
  body?: string;
}
