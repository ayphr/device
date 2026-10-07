import { describe, expect, it } from 'vitest';
import { compareVersions, isNewerVersion, normalizeFirmwareVersion } from './version';

describe('normalizeFirmwareVersion', () => {
  it('strips firmware-v and v prefixes', () => {
    expect(normalizeFirmwareVersion('firmware-v1.2.3')).toBe('1.2.3');
    expect(normalizeFirmwareVersion('FIRMWARE-V1.2.3')).toBe('1.2.3');
    expect(normalizeFirmwareVersion('v1.2.3')).toBe('1.2.3');
    expect(normalizeFirmwareVersion('1.2.3')).toBe('1.2.3');
  });

  it('leaves unrelated text untouched', () => {
    expect(normalizeFirmwareVersion('beta2')).toBe('beta2');
  });
});

describe('compareVersions', () => {
  it('compares numeric core segments', () => {
    expect(compareVersions('1.0.0', '1.0.0')).toBe(0);
    expect(compareVersions('1.0.1', '1.0.0')).toBeGreaterThan(0);
    expect(compareVersions('1.0.0', '1.0.1')).toBeLessThan(0);
    expect(compareVersions('1.10.0', '1.9.0')).toBeGreaterThan(0);
    expect(compareVersions('2.0.0', '10.0.0')).toBeLessThan(0);
  });

  it('treats missing segments as zero', () => {
    expect(compareVersions('1.0', '1.0.0')).toBe(0);
    expect(compareVersions('1', '1.0.0')).toBe(0);
    expect(compareVersions('1.0.1', '1.0')).toBeGreaterThan(0);
  });

  it('falls back to zero instead of NaN for non-numeric parts', () => {
    expect(compareVersions('1.x.0', '1.0.0')).toBe(0);
    expect(compareVersions('1.0.0-rc', 'not-a-version')).toBeGreaterThan(0);
  });

  it('ranks a release above its pre-releases', () => {
    expect(compareVersions('1.0.0', '1.0.0-rc.1')).toBeGreaterThan(0);
    expect(compareVersions('1.0.0-rc.1', '1.0.0')).toBeLessThan(0);
  });

  it('orders pre-release identifiers by semver precedence', () => {
    expect(compareVersions('1.0.0-alpha', '1.0.0-beta')).toBeLessThan(0);
    expect(compareVersions('1.0.0-alpha.1', '1.0.0-alpha.2')).toBeLessThan(0);
    expect(compareVersions('1.0.0-alpha.1', '1.0.0-alpha.beta')).toBeLessThan(0);
    expect(compareVersions('1.0.0-beta', '1.0.0-beta.2')).toBeLessThan(0);
    expect(compareVersions('1.0.0-beta.11', '1.0.0-beta.2')).toBeGreaterThan(0);
    expect(compareVersions('1.0.0-rc.1', '1.0.0-rc.1')).toBe(0);
  });

  it('ignores build metadata', () => {
    expect(compareVersions('1.0.0+build.5', '1.0.0+build.9')).toBe(0);
    expect(compareVersions('1.0.0+build.5', '1.0.1+build.1')).toBeLessThan(0);
  });

  it('normalizes prefixed inputs before comparing', () => {
    expect(compareVersions('firmware-v1.2.3', 'v1.2.2')).toBeGreaterThan(0);
    expect(compareVersions('v1.2.3', 'firmware-v1.2.3')).toBe(0);
  });
});

describe('isNewerVersion', () => {
  it('detects upgrades and rejects downgrades or equal versions', () => {
    expect(isNewerVersion('1.0.1', '1.0.0')).toBe(true);
    expect(isNewerVersion('1.0.0', '1.0.1')).toBe(false);
    expect(isNewerVersion('1.0.0', '1.0.0')).toBe(false);
    expect(isNewerVersion('1.0.0-rc.1', '1.0.0')).toBe(false);
  });
});
