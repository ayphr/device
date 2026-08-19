import {
  IconArrowLeft,
  IconAntennaBars3,
  IconCloudDownload,
  IconDeviceDesktop,
  IconInfoCircle,
  IconLock,
  IconRefresh,
  IconSettings,
  IconTrash,
  IconUpload,
  IconWifi,
  IconWifiOff,
} from '@tabler/icons-react';
import { useState, useEffect, type ComponentType } from 'react';
import { formatLastSeen, formatRssi, rssiLabel, signalStrengthLabel, type DeviceInfo } from '../../lib/devices';
import { Button, ConfirmDialog, Modal, Input } from '../../components/common';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import styles from './Device.module.css';

interface DevicePageProps {
  readonly device: DeviceInfo;
  readonly onBack: () => void;
}

type DeviceSection = 'general' | 'updates' | 'info';

const sections: Array<{ id: DeviceSection; label: string; icon: ComponentType<{ size?: number }> }> = [
  { id: 'general', label: 'General', icon: IconSettings },
  { id: 'updates', label: 'Updates', icon: IconCloudDownload },
  { id: 'info', label: 'Info', icon: IconInfoCircle },
];

interface FirmwareInfoResult {
  version: string;
  hardwareRev: string;
  uptimeSecs: number;
}

interface FirmwareUpdateProgress {
  step: string;
  progress: number;
  message: string;
}

export default function DevicePage({ device, onBack }: Readonly<DevicePageProps>) {
  const [activeSection, setActiveSection] = useState<DeviceSection>('general');
  const [isRestarting, setIsRestarting] = useState(false);
  const [isResetDialogOpen, setIsResetDialogOpen] = useState(false);
  const [isResetting, setIsResetResetting] = useState(false);
  const [isPasswordModalOpen, setIsPasswordModalOpen] = useState(false);
  const [isChangingPassword, setIsChangingPassword] = useState(false);
  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [passwordError, setPasswordError] = useState<string | null>(null);

  const [isWifiModalOpen, setIsWifiModalOpen] = useState(false);
  const [isUpdatingWifi, setIsUpdatingWifi] = useState(false);
  const [wifiSsid, setWifiSsid] = useState('');
  const [wifiPassword, setWifiPassword] = useState('');
  const [wifiError, setWifiError] = useState<string | null>(null);

  const [firmwareInfo, setFirmwareInfo] = useState<FirmwareInfoResult | null>(null);
  const [firmwareError, setFirmwareError] = useState<string | null>(null);
  const [isUpdatingFirmware, setIsUpdatingFirmware] = useState(false);
  const [firmwareProgress, setFirmwareProgress] = useState<FirmwareUpdateProgress | null>(null);
  const [updateAvailable, setUpdateAvailable] = useState<{ version: string; body?: string } | null>(null);
  const [isCheckingFirmwareUpdate, setIsCheckingFirmwareUpdate] = useState(false);

  const activeSectionLabel = sections.find((section) => section.id === activeSection)?.label ?? 'General';

  useEffect(() => {
    let cancelled = false;

    const loadFirmwareInfo = async () => {
      try {
        const command = device.transport === 'serial' ? 'get_firmware_info_serial' : 'get_firmware_info_ble';
        const info = await invoke<FirmwareInfoResult>(command, { deviceId: device.id });
        if (!cancelled) {
          setFirmwareInfo(info);
          setFirmwareError(null);
        }
      } catch (error) {
        if (!cancelled) {
          setFirmwareInfo(null);
          setFirmwareError(typeof error === 'string' ? error : 'Failed to query firmware info');
        }
      }
    };

    void loadFirmwareInfo();

    return () => {
      cancelled = true;
    };
  }, [device.id, device.transport]);

  useEffect(() => {
    let unlistenProgress: (() => void) | undefined;

    const setup = async () => {
      unlistenProgress = await listen<FirmwareUpdateProgress>('firmware-update-progress', (event) => {
        setFirmwareProgress(event.payload);

        if (event.payload.step === 'complete' || event.payload.step === 'error') {
          setIsUpdatingFirmware(false);
        }
      });
    };

    void setup();

    return () => {
      unlistenProgress?.();
    };
  }, []);

  const handleRestart = async () => {
    setIsRestarting(true);
    try {
      await invoke(device.transport === 'serial' ? 'restart_serial_device' : 'restart_ble_device', {
        deviceId: device.id,
      });
    } catch (error) {
      console.error('Failed to restart device:', error);
    } finally {
      setIsRestarting(false);
    }
  };

  const handleFactoryReset = async () => {
    setIsResetResetting(true);
    try {
      await invoke(device.transport === 'serial' ? 'factory_reset_serial_device' : 'factory_reset_ble_device', {
        deviceId: device.id,
      });
      onBack();
    } catch (error) {
      console.error('Failed to factory reset device:', error);
    } finally {
      setIsResetResetting(false);
      setIsResetDialogOpen(false);
    }
  };

  const handleChangePassword = async () => {
    if (device.authRequired === false) return;

    if (newPassword.length < 8) {
      setPasswordError('New password must be at least 8 characters');
      return;
    }
    setIsChangingPassword(true);
    setPasswordError(null);
    try {
      await invoke(device.transport === 'serial' ? 'change_serial_device_password' : 'change_ble_device_password', {
        deviceId: device.id,
        currentPassword,
        newPassword,
      });
      setIsPasswordModalOpen(false);
      setCurrentPassword('');
      setNewPassword('');
    } catch (error) {
      setPasswordError(typeof error === 'string' ? error : 'Failed to change password');
    } finally {
      setIsChangingPassword(false);
    }
  };

  const handleWifiUpdate = async () => {
    if (wifiSsid.trim().length === 0) {
      setWifiError('SSID cannot be empty');
      return;
    }
    setIsUpdatingWifi(true);
    setWifiError(null);
    try {
      await invoke(device.transport === 'serial' ? 'update_serial_device_wifi' : 'update_ble_device_wifi', {
        deviceId: device.id,
        ssid: wifiSsid.trim(),
        password: wifiPassword,
      });
      setIsWifiModalOpen(false);
      setWifiSsid('');
      setWifiPassword('');
    } catch (error) {
      setWifiError(typeof error === 'string' ? error : 'Failed to update Wi-Fi');
    } finally {
      setIsUpdatingWifi(false);
    }
  };

  const handleCheckFirmwareUpdate = async () => {
    setIsCheckingFirmwareUpdate(true);
    setFirmwareError(null);
    try {
      const response = await fetch('https://api.github.com/repos/ayphr/device/releases/latest', {
        headers: { Accept: 'application/vnd.github.v3+json' },
      });
      if (!response.ok) {
        throw new Error('Failed to fetch latest release');
      }
      const release = await response.json();
      const firmwareAsset = release.assets?.find((a: { name: string }) => a.name.endsWith('.bin'));
      if (firmwareAsset) {
        setUpdateAvailable({
          version: release.tag_name?.replace('firmware-v', '') ?? release.tag_name,
          body: release.body ?? undefined,
        });
      } else {
        setUpdateAvailable(null);
      }
    } catch (error) {
      setUpdateAvailable(null);
      setFirmwareError(typeof error === 'string' ? error : 'Failed to check for updates');
    } finally {
      setIsCheckingFirmwareUpdate(false);
    }
  };

  const handleInstallFirmwareUpdate = async () => {
    if (!updateAvailable) return;
    setIsUpdatingFirmware(true);
    setFirmwareProgress(null);
    setFirmwareError(null);
    try {
      const response = await fetch('https://api.github.com/repos/ayphr/device/releases/latest', {
        headers: { Accept: 'application/vnd.github.v3+json' },
      });
      if (!response.ok) throw new Error('Failed to fetch release');
      const release = await response.json();
      const asset = release.assets?.find((a: { name: string }) => a.name.endsWith('.bin'));
      if (!asset) throw new Error('Firmware binary not found in release');

      const command = device.transport === 'serial' ? 'download_and_update_firmware_serial' : 'download_and_update_firmware_ble';
      await invoke(command, {
        deviceId: device.id,
        downloadUrl: asset.browser_download_url,
      });

      setUpdateAvailable(null);
    } catch (error) {
      setFirmwareProgress({ step: 'error', progress: 0, message: typeof error === 'string' ? error : 'Update failed' });
      setIsUpdatingFirmware(false);
    }
  };

  const generalSection = (
    <div className={styles['device-page__stacked-grid']}>
      <article className={styles['device-page__panel']}>
        <h2>System Controls</h2>
        <div className={styles['device-page__control-list']}>
          <Button variant="secondary" onClick={handleRestart} isLoading={isRestarting}>
            <IconRefresh size={16} />
            Restart device
          </Button>
          <Button variant="secondary" onClick={() => setIsWifiModalOpen(true)}>
            <IconWifi size={16} />
            Update Wi-Fi
          </Button>
          {device.authRequired === false ? (
            <p style={{ margin: 0, color: 'var(--color-warning)' }}>
              Authentication is disabled for this device, so password controls are hidden.
            </p>
          ) : (
            <Button variant="secondary" onClick={() => setIsPasswordModalOpen(true)}>
              <IconLock size={16} />
              Change password
            </Button>
          )}
          <Button variant="danger" onClick={() => setIsResetDialogOpen(true)}>
            <IconTrash size={16} />
            Factory reset
          </Button>
        </div>
      </article>

      <ConfirmDialog
        isOpen={isResetDialogOpen}
        title="Factory Reset"
        confirmText="Reset device"
        isDangerous={true}
        isLoading={isResetting}
        onConfirm={handleFactoryReset}
        onCancel={() => setIsResetDialogOpen(false)}
      >
        <p>Are you sure you want to factory reset this device? This will erase all Wi-Fi settings and credentials. You will need to set up the device again.</p>
      </ConfirmDialog>

      <Modal
        isOpen={isPasswordModalOpen}
        title="Change Device Password"
        onClose={() => setIsPasswordModalOpen(false)}
        onConfirm={handleChangePassword}
        isLoading={isChangingPassword}
        confirmText="Update password"
      >
        <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
          <Input
            label="Current password"
            type="password"
            value={currentPassword}
            onChange={(e) => setCurrentPassword(e.target.value)}
            placeholder="Enter current password"
          />
          <Input
            label="New password"
            type="password"
            value={newPassword}
            onChange={(e) => setNewPassword(e.target.value)}
            placeholder="Enter new password (min 8 chars)"
          />
          {passwordError && (
            <p style={{ color: 'var(--color-error)', fontSize: '0.875rem' }}>{passwordError}</p>
          )}
        </div>
      </Modal>

      <Modal
        isOpen={isWifiModalOpen}
        title="Update Wi-Fi Settings"
        onClose={() => setIsWifiModalOpen(false)}
        onConfirm={handleWifiUpdate}
        isLoading={isUpdatingWifi}
        confirmText="Update settings"
      >
        <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
          <Input
            label="Wi-Fi SSID"
            value={wifiSsid}
            onChange={(e) => setWifiSsid(e.target.value)}
            placeholder="Enter network name"
          />
          <Input
            label="Wi-Fi Password"
            type="password"
            value={wifiPassword}
            onChange={(e) => setWifiPassword(e.target.value)}
            placeholder="Enter network password"
          />
          {wifiError && (
            <p style={{ color: 'var(--color-error)', fontSize: '0.875rem' }}>{wifiError}</p>
          )}
        </div>
      </Modal>
    </div>
  );

  const updatesSection = (
    <div className={styles['device-page__stacked-grid']}>
      <article className={styles['device-page__panel']}>
        <h2>Firmware</h2>
        <div className={styles['device-page__setting-list']}>
          <div className={styles['device-page__setting-row']}>
            <div>
              <h3>Current version</h3>
              <p>{firmwareInfo ? `v${firmwareInfo.version}` : firmwareError ?? 'Unknown'}</p>
            </div>
            {firmwareInfo && (
              <span className={styles['device-page__setting-value']}>{firmwareInfo.version}</span>
            )}
          </div>

          <div className={styles['device-page__setting-row']} style={{ marginTop: '0.6rem' }}>
            <div>
              <h3>Check for updates</h3>
              <p>Check if a newer firmware version is available</p>
            </div>
            <div>
              <Button variant="secondary" onClick={handleCheckFirmwareUpdate} disabled={isCheckingFirmwareUpdate || isUpdatingFirmware}>
                {isCheckingFirmwareUpdate ? 'Checking...' : 'Check for updates'}
              </Button>
            </div>
          </div>

          {updateAvailable && !isUpdatingFirmware && (
            <div className={styles['device-page__update-banner']}>
              <div className={styles['device-page__update-info']}>
                <strong>Firmware update available</strong>
                <span>Version {updateAvailable.version} is ready to install</span>
                {updateAvailable.body && (
                  <p className={styles['device-page__update-notes']}>{updateAvailable.body}</p>
                )}
              </div>
              <Button onClick={handleInstallFirmwareUpdate} isLoading={isUpdatingFirmware}>
                <IconUpload size={16} />
                Update now
              </Button>
            </div>
          )}

          {isUpdatingFirmware && firmwareProgress && (
            <div className={styles['device-page__update-progress']}>
              <div className={styles['device-page__progress-header']}>
                <span className={styles['device-page__progress-step']}>{firmwareProgress.message}</span>
                <span className={styles['device-page__progress-pct']}>{Math.round(firmwareProgress.progress)}%</span>
              </div>
              <div className={styles['device-page__progress-bar']}>
                <div
                  className={styles['device-page__progress-fill']}
                  style={{ width: `${firmwareProgress.progress}%` }}
                />
              </div>
              <div className={styles['device-page__progress-steps']}>
                <span className={`${styles['device-page__step']} ${firmwareProgress.step === 'preparing' || firmwareProgress.step === 'sending' || firmwareProgress.step === 'verifying' || firmwareProgress.step === 'complete' ? styles['device-page__step--done'] : ''}`}>
                  Preparing
                </span>
                <span className={`${styles['device-page__step']} ${firmwareProgress.step === 'sending' || firmwareProgress.step === 'verifying' || firmwareProgress.step === 'complete' ? styles['device-page__step--done'] : ''}`}>
                  Sending
                </span>
                <span className={`${styles['device-page__step']} ${firmwareProgress.step === 'verifying' || firmwareProgress.step === 'complete' ? styles['device-page__step--done'] : ''}`}>
                  Verifying
                </span>
                <span className={`${styles['device-page__step']} ${firmwareProgress.step === 'complete' ? styles['device-page__step--done'] : ''}`}>
                  Complete
                </span>
              </div>
            </div>
          )}

          {firmwareProgress?.step === 'error' && (
            <div className={styles['device-page__update-error']}>
              <p>{firmwareProgress.message}</p>
              <Button variant="secondary" onClick={() => setFirmwareProgress(null)}>Dismiss</Button>
            </div>
          )}
        </div>
      </article>
    </div>
  );

  const infoSection = (
    <div className={styles['device-page__stacked-grid']}>
      <div className={styles['device-page__hero']}>
        <div className={styles['device-page__metrics']}>
          <article className={styles['device-page__metric-card']}>
            <span className={styles['device-page__metric-label']}>
              <IconWifiOff size={14} />
              RSSI
            </span>
            <strong>{formatRssi(device.rssi)}</strong>
          </article>

          <article className={styles['device-page__metric-card']}>
            <span className={styles['device-page__metric-label']}>
              <IconAntennaBars3 size={14} />
              Signal
            </span>
            <strong>{signalStrengthLabel(device.signalStrength)}</strong>
          </article>

          <article className={styles['device-page__metric-card']}>
            <span className={styles['device-page__metric-label']}>
              <IconDeviceDesktop size={14} />
              Transport
            </span>
            <strong>{device.transport.toUpperCase()}</strong>
          </article>
        </div>
      </div>

      <article className={styles['device-page__panel']}>
        <h2>Device Details</h2>
        <dl className={`${styles['device-page__definition-list']} ${styles['device-page__definition-list--wide']}`.trim()}>
          <div>
            <dt>Name</dt>
            <dd>{device.name}</dd>
          </div>
          <div>
            <dt>Model</dt>
            <dd>{device.modelId}</dd>
          </div>
          <div>
            <dt>Address</dt>
            <dd>{device.address}</dd>
          </div>
          <div>
            <dt>RSSI</dt>
            <dd>{rssiLabel(device.rssi)} ({formatRssi(device.rssi)})</dd>
          </div>
          <div>
            <dt>Signal</dt>
            <dd>{signalStrengthLabel(device.signalStrength)} ({device.signalStrength}/5 bars)</dd>
          </div>
          <div>
            <dt>Last seen</dt>
            <dd>{formatLastSeen(device.lastSeenSecondsAgo)}</dd>
          </div>
          {firmwareInfo && (
            <>
              <div>
                <dt>Firmware version</dt>
                <dd>v{firmwareInfo.version}</dd>
              </div>
              <div>
                <dt>Hardware rev</dt>
                <dd>{firmwareInfo.hardwareRev}</dd>
              </div>
              <div>
                <dt>Uptime</dt>
                <dd>{firmwareInfo.uptimeSecs}s</dd>
              </div>
            </>
          )}
        </dl>
      </article>

      <article className={styles['device-page__panel']}>
        <h2>Advertisement Data</h2>
        <dl className={`${styles['device-page__definition-list']} ${styles['device-page__definition-list--wide']}`.trim()}>
          <div>
            <dt>Manufacturer data</dt>
            <dd>{device.manufacturerData.length > 0 ? device.manufacturerData.join(', ') : 'None'}</dd>
          </div>
          <div>
            <dt>Service UUIDs</dt>
            <dd>{device.serviceUuids.length > 0 ? device.serviceUuids.join(', ') : 'None'}</dd>
          </div>
        </dl>
      </article>
    </div>
  );

  return (
    <section className={styles['device-page']} aria-label={`${device.name} details`}>
      <div className={styles['device-page__shell']}>
        <aside className={styles['device-page__sidebar']}>
          <button className={styles['device-page__back']} type="button" onClick={onBack}>
            <IconArrowLeft size={16} />
            Back to devices
          </button>

          <nav className={styles['device-page__nav']} aria-label="Device sections">
            {sections.map(({ id, label, icon: Icon }) => (
              <button
                key={id}
                className={`${styles['device-page__nav-item']} ${activeSection === id ? styles['device-page__nav-item--active'] : ''}`.trim()}
                type="button"
                onClick={() => setActiveSection(id)}
                aria-current={activeSection === id ? 'page' : undefined}
              >
                <Icon size={18} />
                {label}
              </button>
            ))}
          </nav>
        </aside>

        <div className={styles['device-page__content']}>
          <div className={styles['device-page__content-header']}>
            <p className={styles['device-page__eyebrow']}>{device.name}</p>
            <h1>{activeSectionLabel}</h1>
          </div>

          {activeSection === 'general' && generalSection}
          {activeSection === 'updates' && updatesSection}
          {activeSection === 'info' && infoSection}
        </div>
      </div>
    </section>
  );
}
