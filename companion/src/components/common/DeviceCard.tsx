import {
  IconAntennaBars1,
  IconAntennaBars2,
  IconAntennaBars3,
  IconAntennaBars4,
  IconAntennaBars5,
  IconAntennaBarsOff,
  IconBluetoothConnected,
  IconBluetoothOff,
} from '@tabler/icons-react';
import styles from './DeviceCard.module.css';

export interface DeviceCardProps {
  name: string;
  onClick?: () => void;
  elevated?: boolean;
  signalStrength: number; // 0 to 5
  connected: boolean;
  setupComplete: boolean;
  subtitle?: string;
  subtitleIcon?: React.ReactNode;
}

export const DeviceCard = ({
  name,
  onClick,
  elevated = false,
  signalStrength,
  connected,
  setupComplete,
  subtitle,
  subtitleIcon,
}: DeviceCardProps) => {
  const signalIcon = (() => {
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
  })();

  const cardBody = (
    <>
      <div className={styles['device-card__meta']}>
        <div className={styles['device-card__text']}>
          <span className={styles['device-card__name']}>{name}</span>
          {subtitle ? (
            <div className={styles['device-card__model-info']}>
              {subtitleIcon ? <span className={styles['device-card__model-icon']}>{subtitleIcon}</span> : null}
              <span className={styles['device-card__model']}>{setupComplete ? subtitle : `Unconfigured ${subtitle}`}</span>
            </div>
          ) : null}
        </div>
        <div className={styles['device-card__status-icons']} aria-label="Device status">
          <span className={styles['device-card__status-icon']} title="Wireless signal">
            {signalIcon}
          </span>
          {connected ? (
            <span className={styles['device-card__status-icon']} title="Bluetooth">
              <IconBluetoothConnected size={16} />
            </span>
          ) : (
            <span className={styles['device-card__status-icon']} title="Not connected">
              <IconBluetoothOff size={16} style={{ opacity: 0.3 }} />
            </span>
          )}
        </div>
      </div>

      <div className={styles['device-card__hero']} role="img" aria-label={`${name} image`}>
        <img
          className={styles['device-card__image']}
          src={`https://picsum.photos/seed/${encodeURIComponent(name)}/420/640`}
          alt={`${name}`}
        />
      </div>
    </>
  );

  if (onClick) {
    return (
      <button
        className={`${styles['device-card']} ${elevated ? styles['device-card--elevated'] : ''} ${styles['device-card--clickable']}`.trim()}
        type="button"
        onClick={onClick}
      >
        {cardBody}
      </button>
    );
  }

  return <div className={`${styles['device-card']} ${elevated ? styles['device-card--elevated'] : ''}`.trim()}>{cardBody}</div>;
};
