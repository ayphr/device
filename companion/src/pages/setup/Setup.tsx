import { useState } from 'react';
import { IconArrowLeft, IconCheck, IconDeviceDesktop, IconShieldCheck, IconWifi } from '@tabler/icons-react';
import { Button, Card, CardBody, CardFooter, CardHeader, Input, Toggle } from '../../components/common';
import { type DeviceInfo } from '../../lib/devices';
import { invoke } from '@tauri-apps/api/core';
import styles from './Setup.module.css';

interface BleConnectionState {
  connected: boolean;
  authenticated: boolean;
  setupComplete: boolean;
  deviceName: string;
}

interface SetupPageProps {
  readonly device: DeviceInfo;
  readonly onBack: () => void;
  readonly onComplete: (device: DeviceInfo) => void;
}

type SetupStep = 'name' | 'wifi' | 'password' | 'terms' | 'review';
type SetupStepWithIntro = 'intro' | SetupStep;

const stepOrder: SetupStepWithIntro[] = ['intro', 'name', 'wifi', 'password', 'terms', 'review'];

const stepTitles: Record<SetupStepWithIntro, string> = {
  intro: 'Set it up',
  name: 'Device name',
  wifi: 'Wi-Fi',
  password: 'Device password',
  terms: 'Terms',
  review: 'Review',
};

const stepIcons: Record<SetupStepWithIntro, typeof IconDeviceDesktop> = {
  intro: IconDeviceDesktop,
  name: IconDeviceDesktop,
  wifi: IconWifi,
  password: IconShieldCheck,
  terms: IconCheck,
  review: IconCheck,
};

export default function SetupPage({ device, onBack, onComplete }: Readonly<SetupPageProps>) {
  const [step, setStep] = useState<SetupStepWithIntro>('intro');
  const [deviceName, setDeviceName] = useState(device.name);
  const [wifiSsid, setWifiSsid] = useState('');
  const [wifiPassword, setWifiPassword] = useState('');
  const [devicePassword, setDevicePassword] = useState('');
  const [agreedToTos, setAgreedToTos] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const currentStepIndex = stepOrder.indexOf(step);
  const isFirstStep = step === 'intro';
  const canContinue =
    step === 'intro' ||
    (step === 'name' && deviceName.trim().length > 0) ||
    (step === 'wifi' && wifiSsid.trim().length > 0 && wifiPassword.trim().length > 0) ||
    (step === 'password' && devicePassword.trim().length >= 8) ||
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
      const connection = await invoke<BleConnectionState>('submit_ble_setup', {
        deviceId: device.id,
        deviceName: deviceName.trim() || device.name,
        wifiSsid: wifiSsid.trim(),
        wifiPassword,
        devicePassword,
      });

      onComplete({
        ...device,
        name: connection.deviceName || deviceName.trim() || device.name,
        setupComplete: connection.setupComplete,
        connected: connection.connected,
        authenticated: connection.authenticated,
      });
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
                  <strong>5 steps</strong>
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
            <p>The device will use these credentials when it joins the local network.</p>
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
          </div>
        );

      case 'password':
        return (
          <div className={styles['setup-page__section']}>
            <h2>Create a device password</h2>
            <p>Use at least 8 characters so the password is easy to remember and hard to guess.</p>
            <Input
              label="Device password"
              type="password"
              value={devicePassword}
              onChange={(event) => setDevicePassword(event.target.value)}
              placeholder="Choose a device password"
              helperText="A longer password is better for shared spaces."
              fullWidth={true}
            />
          </div>
        );

      case 'terms':
        return (
          <div className={styles['setup-page__section']}>
            <h2>Agree to the terms</h2>
            <p>Review the terms of service before continuing to the final overview.</p>
            <div className={`${styles['setup-page__tos-card']} ${agreedToTos ? styles['setup-page__tos-card--active'] : ''}`.trim()}>
              <div>
                <strong>I agree to the terms of service</strong>
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
                <dd>{wifiSsid}</dd>
              </div>
              <div>
                <dt>Device password</dt>
                <dd>{devicePassword.length > 0 ? 'Set' : 'Not set'}</dd>
              </div>
              <div>
                <dt>Terms</dt>
                <dd>{agreedToTos ? 'Accepted' : 'Not accepted'}</dd>
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
                <strong>5 steps</strong>
                <span>Quick local setup</span>
              </div>
              <div>
                <strong>{device.name}</strong>
                <span>Selected device</span>
              </div>
            </div>
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
    </section>
  );
}
