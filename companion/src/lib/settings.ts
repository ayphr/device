export type AppSettings = {
  general: {
    launchOnLogin: boolean;
    stayOpenInBackground: boolean;
    systemNotifications: boolean;
    recommendations: boolean;
  };
  updates: {
    automaticUpdates: boolean;
  };
  accessibility: {
    logoAnimation: boolean;
  };
};

const SETTINGS_STORAGE_KEY = 'ayphr-companion-settings-v1';

export const defaultAppSettings: AppSettings = {
  general: {
    launchOnLogin: true,
    stayOpenInBackground: false,
    systemNotifications: true,
    recommendations: true,
  },
  updates: {
    automaticUpdates: true,
  },
  accessibility: {
    logoAnimation: true,
  },
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function readBoolean(value: unknown, fallback: boolean): boolean {
  return typeof value === 'boolean' ? value : fallback;
}

export function normalizeAppSettings(value: unknown): AppSettings {
  const settings = isRecord(value) ? value : {};
  const general = isRecord(settings.general) ? settings.general : {};
  const updates = isRecord(settings.updates) ? settings.updates : {};
  const accessibility = isRecord(settings.accessibility) ? settings.accessibility : {};

  return {
    general: {
      launchOnLogin: readBoolean(general.launchOnLogin, defaultAppSettings.general.launchOnLogin),
      stayOpenInBackground: readBoolean(general.stayOpenInBackground, defaultAppSettings.general.stayOpenInBackground),
      systemNotifications: readBoolean(general.systemNotifications, defaultAppSettings.general.systemNotifications),
      recommendations: readBoolean(general.recommendations, defaultAppSettings.general.recommendations),
    },
    updates: {
      automaticUpdates: readBoolean(updates.automaticUpdates, defaultAppSettings.updates.automaticUpdates),
    },
    accessibility: {
      logoAnimation: readBoolean(accessibility.logoAnimation, defaultAppSettings.accessibility.logoAnimation),
    },
  };
}

export function loadAppSettings(): AppSettings {
  try {
    const storedSettings = globalThis.localStorage?.getItem(SETTINGS_STORAGE_KEY);

    if (!storedSettings) {
      return defaultAppSettings;
    }

    return normalizeAppSettings(JSON.parse(storedSettings));
  } catch {
    return defaultAppSettings;
  }
}

export function saveAppSettings(settings: AppSettings): void {
  globalThis.localStorage?.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(settings));
}
