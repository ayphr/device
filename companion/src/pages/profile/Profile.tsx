import { Card, CardBody, CardHeader } from '../../components/common';
import styles from './Profile.module.css';

export default function ProfilePage() {
  return (
    <section className={styles['profile-page']} aria-label="Profile">
      <Card elevated className={styles['profile-page__card']}>
        <CardHeader>
          <h1>Profile</h1>
        </CardHeader>
        <CardBody>
          <p>
            Account and cloud profile options are available here. This section will allow users to manage their account details, view subscription information, and access cloud storage settings for their devices.
          </p>
        </CardBody>
      </Card>
    </section>
  );
}
