import { useState } from 'react';
import {
  IconActivity,
  IconAccessible,
  IconArrowsExchange,
  IconHome2,
  IconMessageCircle,
  IconInfoCircle,
  IconGlobe,
  IconBrandGithub,
} from '@tabler/icons-react';
import { Toggle, Button, IconButton } from '../../components/common';
import { type AppSettings } from '../../lib/settings';
import styles from './Settings.module.css';
import { APP_BUILD_NUMBER, APP_VERSION } from '../../lib/appInfo';
import { openUrl } from '@tauri-apps/plugin-opener';

type SettingsSection = 'general' | 'updates' | 'feedback' | 'analytics' | 'accessibility' | 'about';

type SettingsPageProps = {
  settings: AppSettings;
  onSettingsChange: React.Dispatch<React.SetStateAction<AppSettings>>;
  onCheckForUpdates?: () => Promise<void> | (() => void);
  isCheckingForUpdate?: boolean;
};

export default function SettingsPage({ settings, onSettingsChange, onCheckForUpdates, isCheckingForUpdate }: Readonly<SettingsPageProps>) {
  const [activeSection, setActiveSection] = useState<SettingsSection>('general');

  return (
    <section className={styles['settings-page']} aria-label="Settings">
      <aside className={styles['settings-page__sidebar']}>
        <nav className={styles['settings-page__nav']} aria-label="Settings sections">
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
            className={`${styles['settings-page__item']} ${activeSection === 'feedback' ? styles['settings-page__item--active'] : ''}`.trim()}
            type="button"
            onClick={() => setActiveSection('feedback')}
          >
            <IconMessageCircle size={18}></IconMessageCircle>
            Share Feedback
          </button>
          <button
            className={`${styles['settings-page__item']} ${activeSection === 'analytics' ? styles['settings-page__item--active'] : ''}`.trim()}
            type="button"
            onClick={() => setActiveSection('analytics')}
          >
            <IconActivity size={18}></IconActivity>
            Analytics
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
              case 'feedback':
                return 'Share Feedback';
              case 'analytics':
                return 'Analytics';
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
                <p>Download and install updates automatically when available</p>
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
              </div>
            </div>
          </div>
        )}
        {activeSection === 'feedback' && (
          <p>Share product feedback and diagnostics preferences.</p>
        )}
        {activeSection === 'analytics' && (
          <p>Control usage analytics collection and reporting.</p>
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
                icon={<IconGlobe size={16} />}
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
