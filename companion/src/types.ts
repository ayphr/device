export type CurrentPage = 'home' | 'stats' | 'settings' | 'device' | 'setup' | 'auth';

export interface DeviceConnectionState {
  connected: boolean;
  authenticated: boolean;
  authRequired: boolean;
  wifiRequired: boolean;
  setupComplete: boolean;
  deviceName: string;
}

export interface FirmwareInfoResult {
  version: string;
  hardwareRev: string;
  uptimeSecs: number;
}

export interface LiveDataResult {
  packetType: number;
  temperature: number;
  humidity: number;
  airPressure: number;
  serial: number;
}

export interface FirmwareUpdateProgress {
  step: string;
  progress: number;
  message: string;
}

export interface FirmwareReleaseMetadata {
  version?: string;
  binaryUrl?: string;
  sha256?: string;
  signature?: string;
  body?: string;
}
