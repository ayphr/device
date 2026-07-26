import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { IconSettings, IconUserCircle } from '@tabler/icons-react';
import { CurrentPage } from './types';
import Home from './pages/home/Home';
import StatsPage from './pages/stats/Stats';
import SettingsPage from './pages/settings/Settings';
import ProfilePage from './pages/profile/Profile';
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
import { relaunch } from '@tauri-apps/plugin-process';
import styles from './App.module.css';
import logo from './assets/logo.svg';

function App() {
  const [page, setPage] = useState<CurrentPage>('home');
  const [selectedDevice, setSelectedDevice] = useState<DeviceInfo | null>(null);
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [isSearchingForGeoDevices, setIsSearchingForGeoDevices] = useState(true);
  const [settings, setSettings] = useState<AppSettings>(() => loadAppSettings());
  const [availableUpdate, setAvailableUpdate] = useState<Update | null>(null);
  const [isCheckingForUpdate, setIsCheckingForUpdate] = useState(false);
  const [isInstallingUpdate, setIsInstallingUpdate] = useState(false);
  const devicesTabRef = useRef<HTMLButtonElement | null>(null);
  const statsTabRef = useRef<HTMLButtonElement | null>(null);
  const isMountedRef = useRef(true);
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
    return () => {
      isMountedRef.current = false;
    };
  }, []);

  const checkForUpdates = async (options: { autoInstall?: boolean } = {}) => {
    if (import.meta.env.DEV) {
      return;
    }

    const { autoInstall = false } = options;

    if (isMountedRef.current) {
      setIsCheckingForUpdate(true);
    }

    try {
      const update = await check({ timeout: 10000 });

      await new Promise((resolve) => setTimeout(resolve, 750));

      if (!update || !isMountedRef.current) {
        return;
      }

      if (autoInstall) {
        if (isMountedRef.current) {
          setIsInstallingUpdate(true);
        }

        try {
          await update.downloadAndInstall();
          await relaunch();
        } catch (error) {
          console.error('Failed to install app update', error);

          if (isMountedRef.current) {
            setAvailableUpdate(update);
          }
        } finally {
          if (isMountedRef.current) {
            setIsInstallingUpdate(false);
          }
        }
      } else {
        setAvailableUpdate(update);
      }
    } catch (error) {
      console.error('Failed to check for app updates', error);
    } finally {
      if (isMountedRef.current) {
        setIsCheckingForUpdate(false);
      }
    }
  };

  useEffect(() => {
    const timerId = globalThis.setTimeout(() => {
      void checkForUpdates({ autoInstall: settings.updates.automaticUpdates });
    }, 0);

    return () => {
      globalThis.clearTimeout(timerId);
    };
  }, [settings.updates.automaticUpdates]);

  const handleCheckForUpdates = async () => {
    await checkForUpdates();
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

  const shellStyle = {
    '--tab-indicator-left': `${tabIndicator.left}px`,
    '--tab-indicator-width': `${tabIndicator.width}px`,
  } as CSSProperties;

  useEffect(() => {
    let cancelled = false;
    let unlistenDevices: (() => void) | undefined;

    const seedDevices = async () => {
      try {
        const initialDevices = await invoke<DeviceInfo[]>('get_ble_devices');

        if (!cancelled) {
          setDevices(initialDevices);
        }
      } catch (error) {
        console.error('Failed to load BLE devices', error);
      } finally {
        if (!cancelled) {
          setIsSearchingForGeoDevices(false);
        }
      }
    };

    void seedDevices();

    void listen<DeviceInfo[]>('ble-devices-updated', (event) => {
      setDevices(event.payload);
    }).then((unlisten) => {
      if (cancelled) {
        unlisten();
        return;
      }

      unlistenDevices = unlisten;
    });

    return () => {
      cancelled = true;
      unlistenDevices?.();
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

  const openDevice = (device: DeviceInfo) => {
    setSelectedDevice(device);
    if (device.authenticated) {
      setPage('device');
    } else {
      setPage(device.setupComplete ? 'auth' : 'setup');
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
          <IconButton
            className={`${styles['app-icon-button']} ${page === 'profile' ? styles['app-icon-button--active'] : ''}`}
            icon={<IconUserCircle size={18}></IconUserCircle>}
            onClick={() => setPage('profile')}
            aria-label="Account"
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
          />
        )}
        {page === 'profile' && <ProfilePage />}
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
          <DevicePage device={selectedDevice} onBack={goBackToDevices} />
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
            A newer version of Ayphr Companion is ready for your platform.
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
              Upgrade now
            </Button>
          </div>
        </div>
      </Modal>
    </div>
  );
}

export default App;
