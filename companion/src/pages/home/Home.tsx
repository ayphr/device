import { IconSquareRotated, IconSquareRotatedFilled, IconSquareRotatedForbid } from '@tabler/icons-react';
import { type ReactNode } from 'react';
import { DeviceCard } from '../../components/common';
import { DEVICE_MODELS, type DeviceInfo } from '../../lib/devices';
import styles from './Home.module.css';

interface HomeProps {
  readonly devices: DeviceInfo[];
  readonly isSearchingForGeoDevices: boolean;
  readonly onOpenDevice: (device: DeviceInfo) => void;
}

export default function Home({ devices, isSearchingForGeoDevices, onOpenDevice }: Readonly<HomeProps>) {
  function getModelIcon(id: string): ReactNode {
    switch (id) {
      case 'geo-mk1':
        return <IconSquareRotated size={16} />;
      case 'geo-mk2':
        return <IconSquareRotatedFilled size={16} />;
      case 'geo-mk3':
        return <IconSquareRotatedForbid size={16} />;
      default:
        return null;
    }
  }

  return (
    <div className={styles['home-page']}>
      {devices.length > 0 ? (
        <div className={styles['home-page__devices']}>
          {devices.map((device) => (
            <DeviceCard
              key={device.id}
              name={device.name}
              subtitle={DEVICE_MODELS[device.modelId]?.name || 'Unknown device'}
              subtitleIcon={getModelIcon(device.modelId)}
              elevated={true}
              signalStrength={device.signalStrength}
              connected={device.authenticated}
              setupComplete={device.setupComplete}
              onClick={() => onOpenDevice(device)}
            />
          ))}
        </div>
      ) : (
        <div className={styles['home-page__empty-state']}>
          {isSearchingForGeoDevices ? (
            <>
              <h1>Searching for Geo devices</h1>
              <p>Please wait while nearby devices are detected.</p>
            </>
          ) : (
            <>
              <h1>No Geo devices found</h1>
              <p>Please wait for your device to be found and make sure it's powered on and within range.</p>
            </>
          )}
        </div>
      )}
    </div>
  );
}
