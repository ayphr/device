import { describe, expect, it } from 'vitest';
import {
  formatLastSeen,
  formatRssi,
  formatUptime,
  rssiLabel,
  signalStrengthLabel,
} from './devices';

describe('signalStrengthLabel', () => {
  it('maps bar counts to readable labels', () => {
    expect(signalStrengthLabel(5)).toBe('Excellent');
    expect(signalStrengthLabel(4)).toBe('Strong');
    expect(signalStrengthLabel(3)).toBe('Good');
    expect(signalStrengthLabel(2)).toBe('Weak');
    expect(signalStrengthLabel(1)).toBe('Very weak');
    expect(signalStrengthLabel(0)).toBe('No signal');
    expect(signalStrengthLabel(null)).toBe('Unavailable');
  });
});

describe('rssiLabel', () => {
  it('maps dBm ranges to readable labels', () => {
    expect(rssiLabel(-45)).toBe('Excellent');
    expect(rssiLabel(-55)).toBe('Good');
    expect(rssiLabel(-65)).toBe('Fair');
    expect(rssiLabel(-75)).toBe('Weak');
    expect(rssiLabel(-90)).toBe('Very weak');
    expect(rssiLabel(null)).toBe('Unavailable');
  });
});

describe('formatRssi', () => {
  it('renders dBm values and handles missing readings', () => {
    expect(formatRssi(-60)).toBe('-60 dBm');
    expect(formatRssi(null)).toBe('Unavailable');
  });
});

describe('formatLastSeen', () => {
  it('renders relative timestamps', () => {
    expect(formatLastSeen(2)).toBe('Just now');
    expect(formatLastSeen(30)).toBe('30 seconds ago');
    expect(formatLastSeen(90)).toBe('2 minutes ago');
    expect(formatLastSeen(60)).toBe('1 minute ago');
    expect(formatLastSeen(1)).toBe('Just now');
  });
});

describe('formatUptime', () => {
  it('renders seconds, minutes, hours and days', () => {
    expect(formatUptime(45)).toBe('45s');
    expect(formatUptime(65)).toBe('1m 5s');
    expect(formatUptime(120)).toBe('2m');
    expect(formatUptime(3_700)).toBe('1h 1m');
    expect(formatUptime(7_200)).toBe('2h');
    expect(formatUptime(90_000)).toBe('1d 1h');
    expect(formatUptime(86_400)).toBe('1d');
  });
});
