import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { IconArrowLeft } from '@tabler/icons-react';
import { Button, Card, CardBody, CardFooter, CardHeader, Input } from '../../components/common';
import { type DeviceInfo } from '../../lib/devices';
import styles from './Auth.module.css';

interface AuthPageProps {
  readonly device: DeviceInfo;
  readonly onBack: () => void;
  readonly onAuthenticated: (device: DeviceInfo) => void;
}

interface BleConnectionState {
  connected: boolean;
  authenticated: boolean;
  setupComplete: boolean;
  deviceName: string;
}

export default function AuthPage({ device, onBack, onAuthenticated }: Readonly<AuthPageProps>) {
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const submitPassword = async () => {
    if (password.trim().length < 8) {
      setError('Enter at least 8 characters.');
      return;
    }

    setIsSubmitting(true);
    setError(null);

    try {
      const connection = await invoke<BleConnectionState>('authenticate_ble_device', {
        deviceId: device.id,
        password,
      });

      onAuthenticated({
        ...device,
        name: connection.deviceName || device.name,
        setupComplete: connection.setupComplete,
        connected: connection.connected,
        authenticated: connection.authenticated,
      });
    } catch (invokeError) {
      const message =
        invokeError instanceof Error
          ? invokeError.message
          : 'Authentication failed. Check the device password and try again.';
      setError(message);
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <section className={styles['auth-page']} aria-label={`${device.name} authentication`}>
      <Card elevated={true} className={styles['auth-page__card']}>
        <CardHeader>
          <button className={styles['auth-page__back']} type="button" onClick={onBack}>
            <IconArrowLeft size={16} />
            Back
          </button>

          <div className={styles['auth-page__header']}>
            <h1>{device.name}</h1>
            <p>
              Enter the device password to authenticate over Bluetooth and open the device page.
            </p>
          </div>
        </CardHeader>

        <CardBody>
          <div className={styles['auth-page__form']}>
            <Input
              label="Device password"
              type="password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              placeholder="Enter device password"
              error={error ?? undefined}
              fullWidth={true}
            />
          </div>
        </CardBody>

        <CardFooter>
          <Button variant="secondary" type="button" onClick={onBack}>
            Cancel
          </Button>
          <Button type="button" onClick={submitPassword} isLoading={isSubmitting}>
            Authenticate
          </Button>
        </CardFooter>
      </Card>
    </section>
  );
}
