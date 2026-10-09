import { useState, useRef, useEffect } from 'react';
import {
  IconAccessible,
  IconArrowsExchange,
  IconHome2,
  IconInfoCircle,
  IconBrandGithub,
  IconWorld,
} from '@tabler/icons-react';
import { Toggle, Button, IconButton } from '../../components/common';
import { type AppSettings } from '../../lib/settings';
import styles from './Settings.module.css';
import { APP_BUILD_NUMBER, APP_VERSION } from '../../lib/appInfo';
import { openUrl } from '@tauri-apps/plugin-opener';

type SettingsSection = 'general' | 'updates' | 'accessibility' | 'about';

export type UpdateCheckFeedback = {
  type: 'success' | 'error';
  message: string;
};

type SettingsPageProps = {
  settings: AppSettings;
  onSettingsChange: React.Dispatch<React.SetStateAction<AppSettings>>;
  onCheckForUpdates?: () => Promise<void> | (() => void);
  isCheckingForUpdate?: boolean;
  updateCheckFeedback?: UpdateCheckFeedback | null;
};

export default function SettingsPage({ settings, onSettingsChange, onCheckForUpdates, isCheckingForUpdate, updateCheckFeedback }: Readonly<SettingsPageProps>) {
  const [activeSection, setActiveSection] = useState<SettingsSection>('general');
  const navRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    const el = navRef.current;
    if (!el) return;

    const shouldHandle = () => window.matchMedia('(max-width: 820px)').matches;

    const onWheel = (e: WheelEvent) => {
      if (!shouldHandle()) return;
      if (el.scrollWidth <= el.clientWidth) return;
      if (e.deltaY === 0) return;
      e.preventDefault();
      el.scrollLeft += e.deltaY;
    };

    el.addEventListener('wheel', onWheel, { passive: false });
    return () => el.removeEventListener('wheel', onWheel as EventListener);
  }, []);

  return (
    <section className={styles['settings-page']} aria-label="Settings">
      <aside className={styles['settings-page__sidebar']}>
        <nav ref={navRef} className={styles['settings-page__nav']} aria-label="Settings sections">
          <button
            className={`${styles['settings-page__item']} ${activeSection === 'general' ? styles['settings-page__item--active'] : ''}`.trim()}
            type="button"
            onClick={() => setActiveSection('general')}
          >
            <IconHome2 size={18}></IconHome2>
            General
          </button>
          <button
            className={`${styles['settings-page__item']} ${activeSection === 'updates' ? styles['settings-page__item--active'] : ''}`.trim()}
            type="button"
            onClick={() => setActiveSection('updates')}
          >
            <IconArrowsExchange size={18}></IconArrowsExchange>
            Updates
          </button>

          <button
            className={`${styles['settings-page__item']} ${activeSection === 'accessibility' ? styles['settings-page__item--active'] : ''}`.trim()}
            type="button"
            onClick={() => setActiveSection('accessibility')}
          >
            <IconAccessible size={18}></IconAccessible>
            Accessibility
          </button>
          <button
            className={`${styles['settings-page__item']} ${activeSection === 'about' ? styles['settings-page__item--active'] : ''}`.trim()}
            type="button"
            onClick={() => setActiveSection('about')}
          >
            <IconInfoCircle size={18}></IconInfoCircle>
            About
          </button>
        </nav>
      </aside>

      <div className={styles['settings-page__content']}>
        <h3 className={styles['settings-page__title']}>
          {(() => {
            switch (activeSection) {
              case 'general':
                return 'General Settings';
              case 'updates':
                return 'Updates';

              case 'accessibility':
                return 'Accessibility';
              case 'about':
                return 'About';
              default:
                return '';
            }
          })()}
        </h3>
        {activeSection === 'general' && (
          <>
            <div className={styles['settings-page__group']}>
              <div className={styles['settings-page__row']}>
                <div className={styles['settings-page__copy']}>
                  <h4>Launch app on login</h4>
                  <p>Always start after logging in</p>
                </div>
                <Toggle
                  checked={settings.general.launchOnLogin}
                  onChange={() =>
                    onSettingsChange((current) => ({
                      ...current,
                      general: {
                        ...current.general,
                        launchOnLogin: !current.general.launchOnLogin,
                      },
                    }))
                  }
                />
              </div>

              <div className={styles['settings-page__row']}>
                <div className={styles['settings-page__copy']}>
                  <h4>Stay open in background</h4>
                  <p>Keep the app running after the window closes so device sessions can stay alive</p>
                </div>
                <Toggle
                  checked={settings.general.stayOpenInBackground}
                  onChange={() =>
                    onSettingsChange((current) => ({
                      ...current,
                      general: {
                        ...current.general,
                        stayOpenInBackground: !current.general.stayOpenInBackground,
                      },
                    }))
                  }
                />
              </div>
            </div>

            <hr className={styles['settings-page__divider']} />

            <div className={styles['settings-page__group']}>
              <h4 className={styles['settings-page__group-title']}>Notifications</h4>

              <div className={styles['settings-page__row']}>
                <div className={styles['settings-page__copy']}>
                  <h4>System Notifications</h4>
                  <p>Allow desktop notifications</p>
                </div>
                <Toggle
                  checked={settings.general.systemNotifications}
                  onChange={() =>
                    onSettingsChange((current) => ({
                      ...current,
                      general: {
                        ...current.general,
                        systemNotifications: !current.general.systemNotifications,
                      },
                    }))
                  }
                />
              </div>

              <div className={styles['settings-page__row']}>
                <div className={styles['settings-page__copy']}>
                  <h4>Update notifications</h4>
                  <p>Notify you when an update is available after an automatic check</p>
                </div>
                <Toggle
                  checked={settings.general.updateNotifications}
                  onChange={() =>
                    onSettingsChange((current) => ({
                      ...current,
                      general: {
                        ...current.general,
                        updateNotifications: !current.general.updateNotifications,
                      },
                    }))
                  }
                />
              </div>

              <div className={styles['settings-page__row']}>
                <div className={styles['settings-page__copy']}>
                  <h4>Recommendations</h4>
                  <p>Selectively recommend devices and experiences that are relevant to you</p>
                </div>
                <Toggle
                  checked={settings.general.recommendations}
                  onChange={() =>
                    onSettingsChange((current) => ({
                      ...current,
                      general: {
                        ...current.general,
                        recommendations: !current.general.recommendations,
                      },
                    }))
                  }
                />
              </div>
            </div>
          </>
        )}
        {activeSection === 'updates' && (
          <div className={styles['settings-page__group']}>
            <div className={styles['settings-page__row']}>
              <div className={styles['settings-page__copy']}>
                <h4>Automatic updates</h4>
                <p>Check for updates automatically when the app starts</p>
              </div>
              <Toggle
                checked={settings.updates.automaticUpdates}
                onChange={() =>
                  onSettingsChange((current) => ({
                    ...current,
                    updates: {
                      ...current.updates,
                      automaticUpdates: !current.updates.automaticUpdates,
                    },
                  }))
                }
              />
            </div>

            <div className={styles['settings-page__row']} style={{ marginTop: '0.6rem' }}>
              <div className={styles['settings-page__copy']}>
                <h4>Check for updates</h4>
                <p>Manually check for available updates and view release details</p>
              </div>
              <div>
                <Button
                  variant="secondary"
                  onClick={() => onCheckForUpdates?.()}
                  disabled={!!isCheckingForUpdate}
                >
                  {isCheckingForUpdate ? 'Checking…' : 'Check for updates'}
                </Button>
                {updateCheckFeedback ? (
                  <p
                    className={`${styles['settings-page__update-feedback']} ${
                      updateCheckFeedback.type === 'error'
                        ? styles['settings-page__update-feedback--error']
                        : ''
                    }`.trim()}
                    role={updateCheckFeedback.type === 'error' ? 'alert' : 'status'}
                  >
                    {updateCheckFeedback.message}
                  </p>
                ) : null}
              </div>
            </div>
          </div>
        )}

        {activeSection === 'accessibility' && (
          <div className={styles['settings-page__group']}>
            <div className={styles['settings-page__row']}>
              <div className={styles['settings-page__copy']}>
                <h4>Logo animation</h4>
                <p>Use the animated shader effect on the app logo</p>
              </div>
              <Toggle
                checked={settings.accessibility.logoAnimation}
                onChange={() =>
                  onSettingsChange((current) => ({
                    ...current,
                    accessibility: {
                      ...current.accessibility,
                      logoAnimation: !current.accessibility.logoAnimation,
                    },
                  }))
                }
              />
            </div>
          </div>
        )}
        {activeSection === 'about' && (
          <div className={styles['settings-page__group']}>
            <b className={styles['settings-page__about-title']}>Ayphr Companion</b>
            <p className={styles['settings-page__description']}>Version: <span className={styles['settings-page__value']}>{APP_VERSION}</span></p>
            <p className={styles['settings-page__description']}>Build: <span className={styles['settings-page__value']}>{APP_BUILD_NUMBER}</span></p>

            <div className={styles['settings-page__about-links']}>
              <IconButton
                icon={<IconBrandGithub size={16} />}
                onClick={() => openUrl('https://github.com/ayphr/device')}
                borderless={false}
              />
              <IconButton
                icon={<IconWorld size={16} />}
                onClick={() => openUrl('https://ayphr.com')}
                borderless={false}
              />
            </div>
          </div>
        )}
      </div>
    </section>
  );
}
