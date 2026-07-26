export type DeviceModelId = 'geo-gen1' | 'geo-gen2' | 'geo-gen3';

export interface DeviceModel {
  id: DeviceModelId;
  name: string;
}

export interface DeviceInfo {
  id: string;
  name: string;
  modelId: DeviceModelId;
  setupComplete: boolean;
  address: string;
  rssi: number | null;
  signalStrength: number;
  connected: boolean;
  authenticated: boolean;
  connectable: boolean;
  lastSeenSecondsAgo: number;
  txPowerLevel: number | null;
  manufacturerData: string[];
  serviceUuids: string[];
}

export const DEVICE_MODELS: Record<DeviceModelId, DeviceModel> = {
  'geo-gen1': { id: 'geo-gen1', name: 'Geo Gen1' },
  'geo-gen2': { id: 'geo-gen2', name: 'Geo Gen2' },
  'geo-gen3': { id: 'geo-gen3', name: 'Geo Gen3' },
};

export function signalStrengthLabel(signalStrength: number | null) {
  if (signalStrength === null) {
    return 'Unavailable';
  }

  if (signalStrength >= 5) {
    return 'Excellent';
  }

  if (signalStrength === 4) {
    return 'Strong';
  }

  if (signalStrength === 3) {
    return 'Good';
  }

  if (signalStrength === 2) {
    return 'Weak';
  }

  if (signalStrength === 1) {
    return 'Very weak';
  }

  return 'No signal';
}

export function rssiLabel(rssi: number | null) {
  if (rssi === null) {
    return 'Unavailable';
  }

  if (rssi >= -50) {
    return 'Excellent';
  }

  if (rssi >= -60) {
    return 'Good';
  }

  if (rssi >= -70) {
    return 'Fair';
  }

  if (rssi >= -80) {
    return 'Weak';
  }

  return 'Very weak';
}

export function formatLastSeen(secondsAgo: number) {
  if (secondsAgo < 5) {
    return 'Just now';
  }

  if (secondsAgo < 60) {
    return `${secondsAgo} seconds ago`;
  }

  const minutesAgo = Math.max(1, Math.round(secondsAgo / 60));

  return minutesAgo === 1 ? '1 minute ago' : `${minutesAgo} minutes ago`;
}

export function formatRssi(rssi: number | null) {
  if (rssi === null) {
    return 'Unavailable';
  }

  return `${rssi} dBm`;
};
