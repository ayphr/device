import { Card, CardHeader, CardBody } from '../../components/common';
import styles from './Stats.module.css';

export default function StatsPage() {
  return (
    <section className={styles['stats-page']} aria-label="Stats">
      <Card elevated className={styles['stats-page__card']}>
        <CardHeader>
          <h1>Stats</h1>
        </CardHeader>
        <CardBody>
          <p>
            Statistics and performance metrics for connected devices will be displayed here. This section will provide users with insights into their device usage, performance trends, and historical data to help them optimize their experience.
          </p>
        </CardBody>
      </Card>
    </section>
  );
}
