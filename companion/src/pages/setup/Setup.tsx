import { useState } from 'react';
import { IconActivity, IconArrowLeft, IconCheck, IconCloud, IconDeviceDesktop, IconShieldCheck, IconWifi } from '@tabler/icons-react';
import { Button, Card, CardBody, CardFooter, CardHeader, Input, Modal, Toggle } from '../../components/common';
import { type DeviceInfo } from '../../lib/devices';
import { type DeviceConnectionState } from '../../types';
import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import { useCloudLink } from '../../lib/useCloudLink';
import {
  DEFAULT_SAMPLING_INTERVAL_SECS,
  DEFAULT_UPLOAD_INTERVAL_SECS,
  MAX_SAMPLING_INTERVAL_SECS,
  MAX_UPLOAD_INTERVAL_SECS,
  MIN_SAMPLING_INTERVAL_SECS,
  MIN_UPLOAD_INTERVAL_SECS,
  validateIntervals,
} from '../../lib/sync';
import styles from './Setup.module.css';

interface SetupPageProps {
  readonly device: DeviceInfo;
  readonly onBack: () => void;
  readonly onComplete: (device: DeviceInfo) => void;
}

type SetupStep = 'name' | 'wifi' | 'sync' | 'password' | 'cloud' | 'terms' | 'review';
type SetupStepWithIntro = 'intro' | SetupStep;

const stepOrder: SetupStepWithIntro[] = ['intro', 'name', 'wifi', 'sync', 'password', 'cloud', 'terms', 'review'];

const stepTitles: Record<SetupStepWithIntro, string> = {
  intro: 'Set it up',
  name: 'Device name',
  wifi: 'Wi-Fi',
  sync: 'Sync schedule',
  password: 'Device password',
  cloud: 'Ayphr Cloud',
  terms: 'Terms',
  review: 'Review',
};

const stepIcons: Record<SetupStepWithIntro, typeof IconDeviceDesktop> = {
  intro: IconDeviceDesktop,
  name: IconDeviceDesktop,
  wifi: IconWifi,
  sync: IconActivity,
  password: IconShieldCheck,
  cloud: IconCloud,
  terms: IconCheck,
  review: IconCheck,
};

export default function SetupPage({ device, onBack, onComplete }: Readonly<SetupPageProps>) {
  const [step, setStep] = useState<SetupStepWithIntro>('intro');
  const [deviceName, setDeviceName] = useState(device.name);
  const [wifiSsid, setWifiSsid] = useState('');
  const [wifiPassword, setWifiPassword] = useState('');
  const [devicePassword, setDevicePassword] = useState('');
  const [skipWifiSetup, setSkipWifiSetup] = useState(false);
  const [disableAuthentication, setDisableAuthentication] = useState(false);
  const [agreedToTos, setAgreedToTos] = useState(false);
  const [samplingInterval, setSamplingInterval] = useState(DEFAULT_SAMPLING_INTERVAL_SECS);
  const [uploadInterval, setUploadInterval] = useState(DEFAULT_UPLOAD_INTERVAL_SECS);
  const [linkCloud, setLinkCloud] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [pendingDevice, setPendingDevice] = useState<DeviceInfo | null>(null);
  const [isCloudLinking, setIsCloudLinking] = useState(false);

  const cloudLink = useCloudLink(device.id, device.transport);

  const currentStepIndex = stepOrder.indexOf(step);
  const isFirstStep = step === 'intro';
  let passwordSummary = 'Not set';

  if (disableAuthentication) {
    passwordSummary = 'Disabled';
  } else if (devicePassword.length > 0) {
    passwordSummary = 'Set';
  }
  const intervalsError = validateIntervals(samplingInterval, uploadInterval);
  const canContinue =
    step === 'intro' ||
    (step === 'name' && deviceName.trim().length > 0) ||
    (step === 'wifi' && (skipWifiSetup || wifiSsid.trim().length > 0)) ||
    (step === 'sync' && intervalsError === null) ||
    (step === 'password' && (disableAuthentication || devicePassword.trim().length >= 8)) ||
    step === 'cloud' ||
    (step === 'terms' && agreedToTos) ||
    step === 'review';

  const goToNextStep = () => {
    const nextStep = stepOrder[currentStepIndex + 1];

    if (nextStep) {
      setStep(nextStep);
    }
  };

  const goToPreviousStep = () => {
    if (isFirstStep) {
      onBack();
      return;
    }

    const previousStep = stepOrder[currentStepIndex - 1];

    if (previousStep) {
      setStep(previousStep);
    }
  };

  const finishSetup = async () => {
    setIsSubmitting(true);
    setSubmitError(null);

    try {
      const command = device.transport === 'serial' ? 'submit_serial_setup' : 'submit_ble_setup';
      const connection = await invoke<DeviceConnectionState>(command, {
        deviceId: device.id,
        deviceName: deviceName.trim() || device.name,
        wifiSsid: wifiSsid.trim(),
        wifiPassword,
        devicePassword,
        authRequired: !disableAuthentication,
        skipWifi: skipWifiSetup,
        samplingIntervalSecs: samplingInterval,
        uploadIntervalSecs: uploadInterval,
      });

      const updatedDevice = {
        ...device,
        name: connection.deviceName || deviceName.trim() || device.name,
        setupComplete: connection.setupComplete,
        connected: connection.connected,
        authenticated: connection.authenticated,
        authRequired: connection.authRequired,
      };

      if (linkCloud) {
        setPendingDevice(updatedDevice);
        setIsCloudLinking(true);
        await cloudLink.start();
      } else {
        onComplete(updatedDevice);
      }
    } catch (invokeError) {
      console.debug('Setup submission failed', invokeError);
      setSubmitError(typeof invokeError === 'string' ? invokeError : String(invokeError));
    } finally {
      setIsSubmitting(false);
    }
  };

  const renderStepContent = () => {
    switch (step) {
      case 'intro':
        return (
          <Card elevated={true} className={styles['setup-page__intro-card']}>
            <CardHeader>
              <div className={styles['setup-page__intro-header']}>
                <div>
                  <h2>Set it up</h2>
                  <p>
                    This setup flow will guide you through naming the device, joining Wi-Fi, setting a password, and
                    confirming the configuration.
                  </p>
                </div>
              </div>
            </CardHeader>

            <CardBody>
              <div className={styles['setup-page__intro-grid']}>
                <div>
                  <strong>{stepCount} steps</strong>
                  <span>Quick local setup</span>
                </div>
                <div>
                  <strong>{device.name}</strong>
                  <span>Selected device</span>
                </div>
              </div>
            </CardBody>
          </Card>
        );

      case 'name':
        return (
          <div className={styles['setup-page__section']}>
            <h2>Give the device a clear name</h2>
            <p>This label will show up in the companion app once setup is complete.</p>
            <Input
              label="Device name"
              value={deviceName}
              onChange={(event) => setDeviceName(event.target.value)}
              placeholder="Front door sensor"
              fullWidth={true}
            />
          </div>
        );

      case 'wifi':
        return (
          <div className={styles['setup-page__section']}>
            <h2>Enter the Wi-Fi network</h2>
            <p>The device will use these credentials when it joins the local network. You can skip this for an offline setup and add Wi-Fi later.</p>
            <div style={{ margin: '1rem 0', padding: '0.9rem 1rem', border: '1px solid var(--color-border-subtle)', borderRadius: '0.9rem', background: 'var(--color-surface-muted)' }}>
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '1rem' }}>
                <div>
                  <strong>Skip Wi-Fi setup</strong>
                  <p style={{ margin: '0.25rem 0 0', color: 'var(--color-text-secondary)' }}>The device will stay offline until you add Wi-Fi later.</p>
                </div>
                <Toggle color='green' checked={skipWifiSetup} onChange={() => setSkipWifiSetup((current) => !current)} />
              </div>
            </div>
            {!skipWifiSetup ? (
              <div className={styles['setup-page__form-grid']}>
                <Input
                  label="Wi-Fi SSID"
                  value={wifiSsid}
                  onChange={(event) => setWifiSsid(event.target.value)}
                  placeholder="Home Network"
                  fullWidth={true}
                />
                <Input
                  label="Wi-Fi password"
                  type="password"
                  value={wifiPassword}
                  onChange={(event) => setWifiPassword(event.target.value)}
                  placeholder="Enter network password"
                  fullWidth={true}
                />
              </div>
            ) : (
              <p role="note" style={{ color: 'var(--color-warning)' }}>
                Warning: skipping Wi-Fi keeps the device offline until you explicitly add a network later.
              </p>
            )}
          </div>
        );

      case 'sync':
        return (
          <div className={styles['setup-page__section']}>
            <h2>Configure sync schedule</h2>
            <p>These intervals control how often the device samples sensors and uploads telemetry to Ayphr Cloud.</p>
            <div className={styles['setup-page__form-grid']}>
              <Input
                label="Sampling interval (seconds)"
                type="number"
                value={String(samplingInterval)}
                onChange={(event) => setSamplingInterval(Number(event.target.value))}
                helperText={`Between ${MIN_SAMPLING_INTERVAL_SECS} and ${MAX_SAMPLING_INTERVAL_SECS} seconds`}
                fullWidth={true}
              />
              <Input
                label="Upload interval (seconds)"
                type="number"
                value={String(uploadInterval)}
                onChange={(event) => setUploadInterval(Number(event.target.value))}
                helperText={`Between ${MIN_UPLOAD_INTERVAL_SECS} and ${MAX_UPLOAD_INTERVAL_SECS} seconds; must be at least the sampling interval`}
                fullWidth={true}
              />
            </div>
            {intervalsError && (
              <p role="alert" style={{ color: 'var(--color-error)', marginTop: '0.5rem' }}>
                {intervalsError}
              </p>
            )}
          </div>
        );

      case 'password':
        return (
          <div className={styles['setup-page__section']}>
            <h2>Create a device password</h2>
            <p>Use at least 8 characters so the password is easy to remember and hard to guess. You can disable authentication for a trusted local-only device, but that weakens security.</p>
            <div style={{ margin: '1rem 0', padding: '0.9rem 1rem', border: '1px solid var(--color-border-subtle)', borderRadius: '0.9rem', background: 'var(--color-surface-muted)' }}>
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '1rem' }}>
                <div>
                  <strong>Disable authentication</strong>
                  <p style={{ margin: '0.25rem 0 0', color: 'var(--color-text-secondary)' }}>This removes password protection. Only use it on devices that are physically trusted and locally accessible.</p>
                </div>
                <Toggle color='green' checked={disableAuthentication} onChange={() => setDisableAuthentication((current) => !current)} />
              </div>
            </div>
            {!disableAuthentication ? (
              <Input
                label="Device password"
                type="password"
                value={devicePassword}
                onChange={(event) => setDevicePassword(event.target.value)}
                placeholder="Choose a device password"
                helperText="A longer password is better for shared spaces."
                fullWidth={true}
              />
            ) : (
              <p role="note" style={{ color: 'var(--color-warning)' }}>
                Warning: authentication is disabled. Anyone with local access can operate this device without a password.
              </p>
            )}
          </div>
        );

      case 'cloud':
        return (
          <div className={styles['setup-page__section']}>
            <h2>Link to Ayphr Cloud (optional)</h2>
            <p>
              You can link this device to Ayphr Cloud now or later from the device page. Linking uploads
              telemetry so you can view it in the web app.
            </p>
            <div style={{ margin: '1rem 0', padding: '0.9rem 1rem', border: '1px solid var(--color-border-subtle)', borderRadius: '0.9rem', background: 'var(--color-surface-muted)' }}>
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '1rem' }}>
                <div>
                  <strong>Link to Ayphr Cloud</strong>
                  <p style={{ margin: '0.25rem 0 0', color: 'var(--color-text-secondary)' }}>
                    Generate a pairing code after setup. You will need to enter it on the Ayphr website.
                  </p>
                </div>
                <Toggle color='green' checked={linkCloud} onChange={() => setLinkCloud((current) => !current)} />
              </div>
            </div>
          </div>
        );

      case 'terms':
        return (
          <div className={styles['setup-page__section']}>
            <h2>Agree to the terms</h2>
            <p>Review the terms of service and privacy policy before continuing to the final overview.</p>
            <p style={{ margin: '0 0 1rem' }}>
              <button
                type="button"
                style={{ background: 'none', border: 'none', color: 'var(--color-primary)', cursor: 'pointer', padding: 0, textDecoration: 'underline' }}
                onClick={() => void openUrl('https://ayphr.com/policies/terms')}
              >
                Terms of Service
              </button>
              {' · '}
              <button
                type="button"
                style={{ background: 'none', border: 'none', color: 'var(--color-primary)', cursor: 'pointer', padding: 0, textDecoration: 'underline' }}
                onClick={() => void openUrl('https://ayphr.com/policies/privacy')}
              >
                Privacy Policy
              </button>
            </p>
            <div className={`${styles['setup-page__tos-card']} ${agreedToTos ? styles['setup-page__tos-card--active'] : ''}`.trim()}>
              <div>
                <strong>I agree to the Terms of Service and Privacy Policy</strong>
                <span>Required to proceed with setup.</span>
              </div>
              <Toggle color='green' checked={agreedToTos} onChange={() => setAgreedToTos((current) => !current)} />
            </div>
          </div>
        );

      case 'review':
        return (
          <div className={styles['setup-page__section']}>
            <h2>Overview and confirm</h2>
            <p>This is the final summary before setup is marked complete.</p>
            <dl className={styles['setup-page__summary']}>
              <div>
                <dt>Device name</dt>
                <dd>{deviceName.trim() || device.name}</dd>
              </div>
              <div>
                <dt>Wi-Fi SSID</dt>
                <dd>{skipWifiSetup ? 'Skipped for now' : wifiSsid}</dd>
              </div>
              <div>
                <dt>Device password</dt>
                <dd>{passwordSummary}</dd>
              </div>
              <div>
                <dt>Sampling interval</dt>
                <dd>{samplingInterval} seconds</dd>
              </div>
              <div>
                <dt>Upload interval</dt>
                <dd>{uploadInterval} seconds</dd>
              </div>
              <div>
                <dt>Ayphr Cloud</dt>
                <dd>{linkCloud ? 'Link after setup' : 'Not linked'}</dd>
              </div>
              <div>
                <dt>Terms</dt>
                <dd>{agreedToTos ? 'Accepted' : 'Not accepted'}</dd>
              </div>
              <div>
                <dt>Warnings</dt>
                <dd>
                  {disableAuthentication ? 'Authentication is disabled for this device.' : 'Authentication remains enabled.'}
                  {' '}
                  {skipWifiSetup ? 'The device will remain offline until Wi-Fi is added later.' : 'Wi-Fi will be configured during setup.'}
                </dd>
              </div>
            </dl>
          </div>
        );
    }
  };

  const isIntro = step === 'intro';
  const stepCount = stepOrder.length - 1;
  const currentStepNumber = isIntro ? 0 : stepOrder.indexOf(step);
  let primaryAction: () => void = goToNextStep;
  let primaryActionLabel = 'Continue';

  if (isIntro) {
    primaryAction = goToNextStep;
    primaryActionLabel = 'Start setup';
  } else if (step === 'review') {
    primaryAction = () => {
      void finishSetup();
    };
    primaryActionLabel = 'Finish setup';
  }
  const heroTitle = isIntro ? 'Set it up' : `Set up ${device.name}`;
  const heroLead = isIntro
    ? 'This setup flow will guide you through naming the device, joining Wi-Fi, creating a password, and confirming the configuration.'
    : 'Complete the remaining setup steps to finish configuration.';

  return (
    <section className={styles['setup-page']} aria-label={`${device.name} setup`}>
      <div className={styles['setup-page__progress-pill']} aria-label="Setup progress">
        {stepOrder.map((stepId, index) => {
          const StepIcon = stepIcons[stepId];
          const isActive = stepId === step;
          const isComplete = index < currentStepIndex;

          return (
            <div
              key={stepId}
              className={`${styles['setup-page__progress-step']} ${isActive ? styles['setup-page__progress-step--active'] : ''} ${isComplete ? styles['setup-page__progress-step--complete'] : ''}`.trim()}
            >
              <span className={styles['setup-page__progress-icon']} aria-hidden="true">
                <StepIcon size={14} />
              </span>
              <span>{stepTitles[stepId]}</span>
            </div>
          );
        })}
      </div>

      {isIntro ? (
        <Card elevated={true} className={styles['setup-page__intro-card']}>
          <CardHeader>
            <div className={styles['setup-page__intro-header']}>
              <div>
                <h1>{heroTitle}</h1>
                <p>{heroLead}</p>
              </div>
            </div>
          </CardHeader>

          <CardBody>
            <div className={styles['setup-page__intro-grid']}>
              <div>
                <strong>{stepCount} steps</strong>
                <span>Quick local setup</span>
              </div>
              <div>
                <strong>{device.name}</strong>
                <span>Selected device</span>
              </div>
            </div>
            <p style={{ marginTop: '1rem', color: 'var(--color-warning)' }}>
              Security warning: you can disable authentication and skip Wi-Fi during setup. Only do this if you understand the device will be less protected and may remain offline until you configure networking later.
            </p>
          </CardBody>

          <CardFooter>
            <Button variant="secondary" type="button" onClick={goToPreviousStep}>
              Cancel setup
            </Button>
            <Button type="button" onClick={goToNextStep}>
              Start setup
            </Button>
          </CardFooter>
        </Card>
      ) : (
        <Card elevated={true} className={styles['setup-page__flow-card']}>
          <CardHeader>
            <button className={styles['setup-page__back']} type="button" onClick={goToPreviousStep}>
              <IconArrowLeft size={16} />
              Back
            </button>

            <div className={styles['setup-page__flow-header']}>
              <p className={styles['setup-page__eyebrow']}>Device setup</p>
              <h1>{heroTitle}</h1>
              <p className={styles['setup-page__lead']}>{heroLead}</p>
              <p className={styles['setup-page__flow-step']}>Step {currentStepNumber} of {stepCount}</p>
            </div>
          </CardHeader>

          <CardBody>{renderStepContent()}</CardBody>

          <CardFooter>
            <Button variant="secondary" type="button" onClick={goToPreviousStep}>
              Back
            </Button>
            <Button type="button" onClick={primaryAction} disabled={!canContinue} isLoading={step === 'review' && isSubmitting}>
              {primaryActionLabel}
            </Button>
          </CardFooter>

          {submitError ? (
            <p className={styles['setup-page__submit-error']} role="alert">
              {submitError}
            </p>
          ) : null}
        </Card>
      )}
      {isCloudLinking && pendingDevice && (
        <Modal
          isOpen={true}
          title="Link to Ayphr Cloud"
          onClose={() => {
            setIsCloudLinking(false);
            onComplete(pendingDevice);
          }}
          confirmText="Done"
          onConfirm={() => {
            setIsCloudLinking(false);
            onComplete(pendingDevice);
          }}
        >
          <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
            {cloudLink.status?.state === 'pending' && cloudLink.status.code ? (
              <>
                <p>Enter this code on the Ayphr website to link your device:</p>
                <div style={{ textAlign: 'center', fontSize: '2rem', fontFamily: 'monospace', letterSpacing: '0.25rem' }}>
                  {cloudLink.status.code}
                </div>
                <p>Expires in {cloudLink.status.expiresInSecs} seconds.</p>
                <Button variant="secondary" onClick={() => void openUrl('https://ayphr.com/devices')}>
                  Open ayphr.com/devices
                </Button>
              </>
            ) : cloudLink.status?.state === 'linked' ? (
              <p>Your device has been linked to Ayphr Cloud.</p>
            ) : cloudLink.status?.state === 'error' ? (
              <p style={{ color: 'var(--color-error)' }}>
                {cloudLink.status.message ?? cloudLink.error ?? 'Cloud linking failed'}
              </p>
            ) : (
              <p>Preparing a pairing code...</p>
            )}
            {cloudLink.error && cloudLink.status?.state !== 'error' && (
              <p style={{ color: 'var(--color-error)' }}>{cloudLink.error}</p>
            )}
            <Button
              variant="secondary"
              onClick={() => {
                void cloudLink.start();
              }}
              disabled={cloudLink.status?.state === 'pending' || cloudLink.isStarting}
            >
              {cloudLink.status?.state === 'pending' ? 'Waiting for link...' : 'Generate new code'}
            </Button>
          </div>
        </Modal>
      )}
    </section>
  );
}
