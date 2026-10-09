import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { IconSettings } from '@tabler/icons-react';
import { CurrentPage, type DeviceConnectionState } from './types';
import Home from './pages/home/Home';
import StatsPage from './pages/stats/Stats';
import SettingsPage, { type UpdateCheckFeedback } from './pages/settings/Settings';
import DevicePage from './pages/device/Device';
import SetupPage from './pages/setup/Setup';
import AuthPage from './pages/auth/Auth';
import { Button, Modal, IconButton } from './components/common';
import { LiquidChromeLogo } from 'liquidity-react';
import { type DeviceInfo } from './lib/devices';
import { loadAppSettings, saveAppSettings, type AppSettings } from './lib/settings';
import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { check, type Update } from '@tauri-apps/plugin-updater';
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification';
import { relaunch } from '@tauri-apps/plugin-process';
import styles from './App.module.css';
import logo from './assets/logo.svg';

const AUTOMATIC_UPDATE_INTERVAL_MS = 60 * 60 * 1000;

function replaceDevicesForTransport(
  currentDevices: DeviceInfo[],
  incomingDevices: DeviceInfo[],
  transport: DeviceInfo['transport'],
) {
  const retainedDevices = currentDevices.filter((device) => device.transport !== transport);
  return [...retainedDevices, ...incomingDevices];
}

function mergeInitialDevices(...deviceGroups: DeviceInfo[][]) {
  return deviceGroups.flat();
}

function App() {
  const [page, setPage] = useState<CurrentPage>('home');
  const [selectedDevice, setSelectedDevice] = useState<DeviceInfo | null>(null);
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [isSearchingForGeoDevices, setIsSearchingForGeoDevices] = useState(true);
  const [settings, setSettings] = useState<AppSettings>(() => loadAppSettings());
  const [availableUpdate, setAvailableUpdate] = useState<Update | null>(null);
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [isCheckingForUpdate, setIsCheckingForUpdate] = useState(false);
  const [isInstallingUpdate, setIsInstallingUpdate] = useState(false);
  const [updateCheckFeedback, setUpdateCheckFeedback] = useState<UpdateCheckFeedback | null>(null);
  const devicesTabRef = useRef<HTMLButtonElement | null>(null);
  const statsTabRef = useRef<HTMLButtonElement | null>(null);
  const isMountedRef = useRef(true);
  const presentedUpdateVersionRef = useRef<string | null>(null);
  const [tabIndicator, setTabIndicator] = useState({ left: 0, width: 0 });

  const isPrimaryPage = page === 'home' || page === 'stats';

  useEffect(() => {
    const updateTabIndicator = () => {
      if (!isPrimaryPage) {
        setTabIndicator({ left: 0, width: 0 });
        return;
      }

      const activeTab = page === 'home' ? devicesTabRef.current : statsTabRef.current;

      if (!activeTab) {
        setTabIndicator({ left: 0, width: 0 });
        return;
      }

      setTabIndicator({
        left: activeTab.offsetLeft,
        width: activeTab.offsetWidth,
      });
    };

    updateTabIndicator();
    window.addEventListener('resize', updateTabIndicator);

    return () => {
      window.removeEventListener('resize', updateTabIndicator);
    };
  }, [isPrimaryPage, page]);

  useEffect(() => {
    saveAppSettings(settings);
  }, [settings]);

  useEffect(() => {
    isMountedRef.current = true;

    return () => {
      isMountedRef.current = false;
    };
  }, []);

  const sendUpdateNotification = async (version: string) => {
    try {
      let permissionGranted = await isPermissionGranted();

      if (!permissionGranted) {
        const permission = await requestPermission();
        permissionGranted = permission === 'granted';
      }

      if (!permissionGranted) {
        return;
      }

      sendNotification({
        title: 'Update available',
        body: `Ayphr Companion ${version} is ready to install.`,
      });
    } catch (error) {
      console.error('Failed to send update notification', error);
    }
  };

  const checkForUpdates = async (options: { notify?: boolean; manual?: boolean } = {}) => {
    if (import.meta.env.DEV) {
      return;
    }

    const { notify = false, manual = false } = options;

    if (isMountedRef.current) {
      setIsCheckingForUpdate(true);
      setUpdateCheckFeedback(null);
    }

    try {
      const update = await check({ timeout: 10000 });

      await new Promise((resolve) => setTimeout(resolve, 750));

      if (!isMountedRef.current) {
        return;
      }

      if (!update) {
        setUpdateCheckFeedback({ type: 'success', message: 'You’re up to date.' });
        return;
      }

      if (!manual && presentedUpdateVersionRef.current === update.version) {
        return;
      }

      presentedUpdateVersionRef.current = update.version;
      setAvailableUpdate(update);

      if (notify) {
        await sendUpdateNotification(update.version);
      }
    } catch (error) {
      console.error('Failed to check for app updates', error);

      if (isMountedRef.current) {
        setUpdateCheckFeedback({
          type: 'error',
          message: 'Couldn’t check for updates. Check your connection and try again.',
        });
      }
    } finally {
      if (isMountedRef.current) {
        setIsCheckingForUpdate(false);
      }
    }
  };

  useEffect(() => {
    if (!settings.updates.automaticUpdates) {
      return;
    }

    const runCheck = () => {
      if (!isMountedRef.current) {
        return;
      }

      void checkForUpdates({
        notify: settings.general.systemNotifications && settings.general.updateNotifications,
      });
    };

    const initialTimerId = globalThis.setTimeout(runCheck, 0);
    const intervalId = globalThis.setInterval(runCheck, AUTOMATIC_UPDATE_INTERVAL_MS);

    return () => {
      globalThis.clearTimeout(initialTimerId);
      globalThis.clearInterval(intervalId);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings.updates.automaticUpdates]);

  const handleCheckForUpdates = async () => {
    await checkForUpdates({ manual: true });
  };

  useEffect(() => {
    let cancelled = false;

    const syncLaunchOnLogin = async () => {
      try {
        const autostartEnabled = await isEnabled();

        if (cancelled) {
          return;
        }

        if (settings.general.launchOnLogin && !autostartEnabled) {
          await enable();
        } else if (!settings.general.launchOnLogin && autostartEnabled) {
          await disable();
        }
      } catch (error) {
        console.error('Failed to sync launch on login', error);
      }
    };

    void syncLaunchOnLogin();

    return () => {
      cancelled = true;
    };
  }, [settings.general.launchOnLogin]);

  useEffect(() => {
    const syncBackgroundMode = async () => {
      try {
        await invoke('set_background_mode', {
          enabled: settings.general.stayOpenInBackground,
        });
      } catch (error) {
        console.error('Failed to sync background mode', error);
      }
    };

    void syncBackgroundMode();
  }, [settings.general.stayOpenInBackground]);

  const shellStyle = {
    '--tab-indicator-left': `${tabIndicator.left}px`,
    '--tab-indicator-width': `${tabIndicator.width}px`,
  } as CSSProperties;

  useEffect(() => {
    let cancelled = false;
    let unlistenDevices: (() => void) | undefined;
    let unlistenSerialDevices: (() => void) | undefined;

    const seedDevices = async () => {
      try {
        const [bleDevices, serialDevices] = await Promise.all([
          invoke<DeviceInfo[]>('get_ble_devices'),
          invoke<DeviceInfo[]>('get_serial_devices'),
        ]);

        if (!cancelled) {
          setDevices(mergeInitialDevices(bleDevices, serialDevices));
        }
      } catch (error) {
        console.error('Failed to load devices', error);
      } finally {
        if (!cancelled) {
          setIsSearchingForGeoDevices(false);
        }
      }
    };

    void seedDevices();

    void listen<DeviceInfo[]>('ble-devices-updated', (event) => {
      setDevices((currentDevices) =>
        replaceDevicesForTransport(currentDevices, event.payload, 'ble'),
      );
    }).then((unlisten) => {
      if (cancelled) {
        unlisten();
        return;
      }

      unlistenDevices = unlisten;
    });

    void listen<DeviceInfo[]>('serial-devices-updated', (event) => {
      setDevices((currentDevices) =>
        replaceDevicesForTransport(currentDevices, event.payload, 'serial'),
      );
    }).then((unlisten) => {
      if (cancelled) {
        unlisten();
        return;
      }

      unlistenSerialDevices = unlisten;
    });

    return () => {
      cancelled = true;
      unlistenDevices?.();
      unlistenSerialDevices?.();
    };
  }, []);

  useEffect(() => {
    if (!selectedDevice) {
      return;
    }

    const refreshedDevice = devices.find((device) => device.id === selectedDevice.id);

    if (refreshedDevice && refreshedDevice !== selectedDevice) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setSelectedDevice(refreshedDevice);
      return;
    }

    if (!refreshedDevice && (page === 'device' || page === 'setup' || page === 'auth')) {
      setSelectedDevice(null);
      setPage('home');
    }
  }, [devices, page, selectedDevice]);

  const openDevice = async (device: DeviceInfo) => {
    setSelectedDevice(device);

    if (!device.setupComplete) {
      setPage('setup');
      return;
    }

    try {
      const connection = await invoke<DeviceConnectionState>(
        device.transport === 'serial' ? 'connect_serial_device' : 'connect_ble_device',
        { deviceId: device.id },
      );

      const updatedDevice = {
        ...device,
        name: connection.deviceName || device.name,
        setupComplete: connection.setupComplete,
        connected: connection.connected,
        authenticated: connection.authenticated,
        authRequired: connection.authRequired,
      };

      setSelectedDevice(updatedDevice);

      if (device.transport === 'serial' || !connection.authRequired || connection.authenticated) {
        setPage('device');
      } else {
        setPage('auth');
      }
    } catch (error) {
      console.error('Failed to connect to device before routing', error);

      setConnectionError(
        typeof error === 'string'
          ? error
          : error instanceof Error
            ? error.message
            : 'Could not reach the device. Make sure it is powered on and nearby, then try again.',
      );

      if (device.authenticated) {
        setPage('device');
      } else {
        setPage('auth');
      }
    }
  };

  const goBackToDevices = () => {
    setPage('home');
  };

  const dismissUpdate = () => {
    if (!isInstallingUpdate) {
      setAvailableUpdate(null);
    }
  };

  const installUpdate = async () => {
    if (!availableUpdate) {
      return;
    }

    const update = availableUpdate;
    setIsInstallingUpdate(true);
    setAvailableUpdate(null);

    try {
      await update.downloadAndInstall();
      await relaunch();
    } catch (error) {
      console.error('Failed to install app update', error);
      setAvailableUpdate(update);
      setIsInstallingUpdate(false);
    }
  };

  const completeDeviceAuthSetup = (updatedDevice: DeviceInfo) => {
    setDevices((currentDevices) =>
      currentDevices.map((device) => (device.id === updatedDevice.id ? updatedDevice : device)),
    );
    setSelectedDevice(updatedDevice);
    setPage('device');
  };

  const handleDeviceUpdated = (updatedDevice: DeviceInfo) => {
    setDevices((currentDevices) =>
      currentDevices.map((device) => (device.id === updatedDevice.id ? updatedDevice : device)),
    );
    setSelectedDevice(updatedDevice);
  };

  return (
    <div
      className={styles['app-shell']}
      style={shellStyle}
      onContextMenu={(event) => event.preventDefault()}
    >
      <header className={styles['app-topbar']}>
        <div className={styles['app-topbar__left']}>
          <span className={styles['brand-mark']}>
            {settings.accessibility.logoAnimation ? (
              <LiquidChromeLogo
                svg={logo}
                size={100}
                speed={0.25}
                noiseIntensity={0}
                scale={4}
                dotFactor={1.2}
                dotMultiplier={0.02}
                vOffset={5}
                intensityFactor={0.5}
                expFactor={0.1}
                redFactor={3}
                greenFactor={3}
                blueFactor={3}
                colorShift={0}
                logoInteractStrength={0.65}
                className={styles['brand-mark__img']}
              />
            ) : (
              <img src={logo} className={styles['brand-mark__img']} alt="Ayphr logo" />
            )}
          </span>

          <nav
            className={`${styles['app-tabs']} ${!isPrimaryPage ? styles['app-tabs--indicator-hidden'] : ''}`}
            aria-label="Primary"
          >
            <button
              ref={devicesTabRef}
              className={`${styles['app-tabs__item']} ${page === 'home' ? styles['app-tabs__item--active'] : ''}`}
              type="button"
              onClick={() => setPage('home')}
              aria-current={page === 'home' ? 'page' : undefined}
            >
              Devices
            </button>
            <button
              ref={statsTabRef}
              className={`${styles['app-tabs__item']} ${page === 'stats' ? styles['app-tabs__item--active'] : ''}`}
              type="button"
              onClick={() => setPage('stats')}
              aria-current={page === 'stats' ? 'page' : undefined}
            >
              Stats
            </button>
            <span className={styles['app-tabs__indicator']} aria-hidden="true"></span>
          </nav>
        </div>

        <div className={styles['app-topbar__right']}>
          <IconButton
            className={`${styles['app-icon-button']} ${page === 'settings' ? styles['app-icon-button--active'] : ''}`}
            icon={<IconSettings size={18}></IconSettings>}
            onClick={() => setPage('settings')}
            aria-label="Settings"
          ></IconButton>

        </div>
      </header>

      <main className={styles['app-content']}>
        {page === 'home' && (
          <Home
            devices={devices}
            isSearchingForGeoDevices={isSearchingForGeoDevices}
            onOpenDevice={openDevice}
          />
        )}
        {page === 'stats' && <StatsPage />}
        {page === 'settings' && (
          <SettingsPage
            settings={settings}
            onSettingsChange={setSettings}
            onCheckForUpdates={handleCheckForUpdates}
            isCheckingForUpdate={isCheckingForUpdate}
            updateCheckFeedback={updateCheckFeedback}
          />
        )}

        {page === 'setup' && selectedDevice ? (
          <SetupPage
            key={selectedDevice.id}
            device={selectedDevice}
            onBack={goBackToDevices}
            onComplete={completeDeviceAuthSetup}
          />
        ) : null}
        {page === 'auth' && selectedDevice ? (
          <AuthPage
            key={selectedDevice.id}
            device={selectedDevice}
            onBack={goBackToDevices}
            onAuthenticated={completeDeviceAuthSetup}
          />
        ) : null}
        {page === 'device' && selectedDevice ? (
          <DevicePage
            device={selectedDevice}
            onBack={goBackToDevices}
            onDeviceUpdated={handleDeviceUpdated}
          />
        ) : null}
      </main>

      <Modal
        isOpen={availableUpdate !== null}
        onClose={dismissUpdate}
        title="Update available"
        showCancel={false}
        disableClose={isInstallingUpdate}
        size="sm"
      >
        <div className={styles['update-modal__content']}>
          <p className={styles['update-modal__copy']}>
            A newer version of Ayphr Companion is available for your platform.
          </p>

          <div className={styles['update-modal__versions']}>
            <div className={styles['update-modal__version-card']}>
              <span className={styles['update-modal__label']}>Current version</span>
              <strong className={styles['update-modal__version']}>{availableUpdate?.currentVersion}</strong>
            </div>

            <div className={styles['update-modal__version-card']}>
              <span className={styles['update-modal__label']}>Upgrade to</span>
              <strong className={styles['update-modal__version']}>{availableUpdate?.version}</strong>
            </div>
          </div>

          {availableUpdate?.body ? (
            <p className={styles['update-modal__notes']}>{availableUpdate.body}</p>
          ) : null}

          {isCheckingForUpdate ? (
            <p className={styles['update-modal__status']}>Checking release details...</p>
          ) : null}

          <div className={styles['update-modal__actions']}>
            <Button variant="secondary" onClick={dismissUpdate} disabled={isInstallingUpdate}>
              Later
            </Button>
            <Button onClick={installUpdate} isLoading={isInstallingUpdate}>
              Update now
            </Button>
          </div>
        </div>
      </Modal>
      <Modal
        isOpen={connectionError !== null}
        onClose={() => setConnectionError(null)}
        title="Connection failed"
        showCancel={false}
        size="sm"
      >
        <div className={styles['update-modal__content']}>
          <p className={styles['update-modal__copy']}>{connectionError}</p>

          <div className={styles['update-modal__actions']}>
            <Button onClick={() => setConnectionError(null)}>Dismiss</Button>
          </div>
        </div>
      </Modal>
    </div>
  );
}

export default App;
