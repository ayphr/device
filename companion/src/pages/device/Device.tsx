import {
  IconAdjustmentsHorizontal,
  IconArrowLeft,
  IconAntennaBars3,
  IconClock,
  IconDatabase,
  IconGauge,
  IconSettings,
  IconWifiOff,
  IconRefresh,
  IconTrash,
  IconLock,
  IconWifi,
  IconAntennaBarsOff,
  IconAntennaBars1,
  IconAntennaBars2,
  IconAntennaBars4,
  IconAntennaBars5,
} from '@tabler/icons-react';
import { useState, type ComponentType } from 'react';
import { formatLastSeen, formatRssi, rssiLabel, signalStrengthLabel, type DeviceInfo } from '../../lib/devices';
import { Button, ConfirmDialog, Modal, Input } from '../../components/common';
import { invoke } from '@tauri-apps/api/core';
import styles from './Device.module.css';

interface DevicePageProps {
  readonly device: DeviceInfo;
  readonly onBack: () => void;
}

type DeviceSection = 'dashboard' | 'controls' | 'data' | 'settings';

const sections: Array<{ id: DeviceSection; label: string; icon: ComponentType<{ size?: number }> }> = [
  { id: 'dashboard', label: 'Dashboard', icon: IconGauge },
  { id: 'controls', label: 'Controls', icon: IconAdjustmentsHorizontal },
  { id: 'data', label: 'Data', icon: IconDatabase },
  { id: 'settings', label: 'Settings', icon: IconSettings },
];

export default function DevicePage({ device, onBack }: Readonly<DevicePageProps>) {
  const [activeSection, setActiveSection] = useState<DeviceSection>('dashboard');
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

  const activeSectionLabel = sections.find((section) => section.id === activeSection)?.label ?? 'Dashboard';

  const signalIcon = (signalStrength: number | null) => {
    if (signalStrength === null) {
      return <IconAntennaBarsOff size={16} style={{ opacity: 0.3 }} />;
    }

    if (signalStrength >= 5) {
      return <IconAntennaBars5 size={16} />;
    }

    if (signalStrength === 4) {
      return <IconAntennaBars4 size={16} />;
    }

    if (signalStrength === 3) {
      return <IconAntennaBars3 size={16} />;
    }

    if (signalStrength === 2) {
      return <IconAntennaBars2 size={16} />;
    }

    if (signalStrength === 1) {
      return <IconAntennaBars1 size={16} />;
    }

    return <IconAntennaBarsOff size={16} style={{ opacity: 0.3 }} />;
  };

  const handleRestart = async () => {
    setIsRestarting(true);
    try {
      await invoke('restart_ble_device', { deviceId: device.id });
    } catch (error) {
      console.error('Failed to restart device:', error);
    } finally {
      setIsRestarting(false);
    }
  };

  const handleFactoryReset = async () => {
    setIsResetResetting(true);
    try {
      await invoke('factory_reset_ble_device', { deviceId: device.id });
      onBack();
    } catch (error) {
      console.error('Failed to factory reset device:', error);
    } finally {
      setIsResetResetting(false);
      setIsResetDialogOpen(false);
    }
  };

  const handleChangePassword = async () => {
    if (newPassword.length < 8) {
      setPasswordError('New password must be at least 8 characters');
      return;
    }
    setIsChangingPassword(true);
    setPasswordError(null);
    try {
      await invoke('change_ble_device_password', {
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
      await invoke('update_ble_device_wifi', {
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

  const dashboardSection = (
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
              <IconClock size={14} />
              Last seen
            </span>
            <strong>{formatLastSeen(device.lastSeenSecondsAgo)}</strong>
          </article>

          <article className={styles['device-page__metric-card']}>
            <span className={styles['device-page__metric-label']}>
              {signalIcon(device.signalStrength)}
              Signal
            </span>
            <strong>{signalStrengthLabel(device.signalStrength)}</strong>
          </article>
        </div>
      </div>
    </div>
  );

  const controlsSection = (
    <div className={styles['device-page__stacked-grid']}>
      <article className={styles['device-page__panel']}>
        <h2>System Controls</h2>
        <div className={styles['device-page__control-list']}>
          <Button
            variant="secondary"
            onClick={handleRestart}
            isLoading={isRestarting}
          >
            <IconRefresh size={16} />
            Restart device
          </Button>
          <Button
            variant="secondary"
            onClick={() => setIsWifiModalOpen(true)}
          >
            <IconWifi size={16} />
            Update Wi-Fi
          </Button>
          <Button
            variant="secondary"
            onClick={() => setIsPasswordModalOpen(true)}
          >
            <IconLock size={16} />
            Change password
          </Button>
          <Button
            variant="danger"
            onClick={() => setIsResetDialogOpen(true)}
          >
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

  const dataSection = (
    <div className={styles['device-page__stacked-grid']}>
      <article className={styles['device-page__panel']}>
        <h2>Data</h2>
        <dl className={`${styles['device-page__definition-list']} ${styles['device-page__definition-list--wide']}`.trim()}>
          <div>
            <dt>RSSI</dt>
            <dd>{rssiLabel(device.rssi)} ({formatRssi(device.rssi)})</dd>
          </div>
          <div>
            <dt>Signal</dt>
            <dd>{signalStrengthLabel(device.signalStrength)} ({device.signalStrength}/5 bars)</dd>
          </div>
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

      <article className={styles['device-page__panel']}>
        <h2>Recent readings</h2>
        <div className={styles['device-page__reading-list']}>
          <div>
            <span>Advertisement</span>
            <strong>{device.connected ? 'Visible' : 'Not visible'}</strong>
          </div>
          <div>
            <span>Last seen</span>
            <strong>{formatLastSeen(device.lastSeenSecondsAgo)}</strong>
          </div>
        </div>
      </article>
    </div>
  );

  const settingsSection = (
    <div className={styles['device-page__stacked-grid']}>
      <article className={styles['device-page__panel']}>
        <h2>Settings</h2>
        <div className={styles['device-page__setting-list']}>
          <div className={styles['device-page__setting-row']}>
            <div>
              <h3>Auto refresh</h3>
              <p>Keep the BLE scan stream updated as new advertisements are discovered.</p>
            </div>
            <span className={styles['device-page__setting-value']}>On</span>
          </div>
          <div className={styles['device-page__setting-row']}>
            <div>
              <h3>Refresh window</h3>
              <p>Retain devices for 30 seconds after their last observed advertisement.</p>
            </div>
            <span className={styles['device-page__setting-value']}>30s</span>
          </div>
          <div className={styles['device-page__setting-row']}>
            <div>
              <h3>Notifications</h3>
              <p>Send alerts when a visible advertiser disappears or a new device appears.</p>
            </div>
            <span className={styles['device-page__setting-value']}>On</span>
          </div>
        </div>
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

          {activeSection === 'dashboard' && dashboardSection}
          {activeSection === 'controls' && controlsSection}
          {activeSection === 'data' && dataSection}
          {activeSection === 'settings' && settingsSection}
        </div>
      </div>
    </section>
  );
}
