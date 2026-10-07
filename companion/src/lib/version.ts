/** Strips `firmware-v`/`v` prefixes so versions can be compared as plain semver. */
export function normalizeFirmwareVersion(version: string): string {
  return version.replace(/^firmware-v/i, '').replace(/^v/i, '');
}

interface ParsedVersion {
  core: number[];
  preRelease: string[];
}

function parseVersion(version: string): ParsedVersion {
  const withoutBuild = version.split('+')[0];
  const separatorIndex = withoutBuild.indexOf('-');
  const corePart = separatorIndex === -1 ? withoutBuild : withoutBuild.slice(0, separatorIndex);
  const preReleasePart = separatorIndex === -1 ? '' : withoutBuild.slice(separatorIndex + 1);

  const core = corePart.split('.').map((part) => {
    const parsed = Number.parseInt(part, 10);
    return Number.isNaN(parsed) ? 0 : parsed;
  });

  const preRelease = preReleasePart.length > 0 ? preReleasePart.split('.') : [];

  return { core, preRelease };
}

function compareCore(a: number[], b: number[]): number {
  const length = Math.max(a.length, b.length);

  for (let index = 0; index < length; index += 1) {
    const diff = (a[index] ?? 0) - (b[index] ?? 0);
    if (diff !== 0) return diff;
  }

  return 0;
}

function isNumeric(identifier: string): boolean {
  return /^[0-9]+$/.test(identifier);
}

function comparePreRelease(a: string[], b: string[]): number {
  // A release outranks any pre-release of the same core version.
  if (a.length === 0 && b.length === 0) return 0;
  if (a.length === 0) return 1;
  if (b.length === 0) return -1;

  const length = Math.max(a.length, b.length);

  for (let index = 0; index < length; index += 1) {
    const left = a[index];
    const right = b[index];

    // A smaller set of pre-release identifiers has higher precedence.
    if (left === undefined) return -1;
    if (right === undefined) return 1;

    const leftIsNumeric = isNumeric(left);
    const rightIsNumeric = isNumeric(right);

    if (leftIsNumeric && rightIsNumeric) {
      const diff = Number.parseInt(left, 10) - Number.parseInt(right, 10);
      if (diff !== 0) return diff < 0 ? -1 : 1;
      continue;
    }

    if (leftIsNumeric !== rightIsNumeric) {
      // Numeric identifiers always have lower precedence than alphanumeric.
      return leftIsNumeric ? -1 : 1;
    }

    if (left !== right) return left < right ? -1 : 1;
  }

  return 0;
}

/**
 * Compares two semantic versions, returning a negative number when `a` sorts
 * before `b`, 0 when they are equal, and a positive number otherwise.
 *
 * Pre-release identifiers follow the semver precedence rules (build metadata
 * is ignored) and non-numeric parts fall back to 0 instead of `NaN`.
 */
export function compareVersions(a: string, b: string): number {
  const left = parseVersion(normalizeFirmwareVersion(a));
  const right = parseVersion(normalizeFirmwareVersion(b));

  const coreDiff = compareCore(left.core, right.core);
  if (coreDiff !== 0) return coreDiff < 0 ? -1 : 1;

  return comparePreRelease(left.preRelease, right.preRelease);
}

/** Whether `candidate` is strictly newer than `current`. */
export function isNewerVersion(candidate: string, current: string): boolean {
  return compareVersions(candidate, current) > 0;
}
