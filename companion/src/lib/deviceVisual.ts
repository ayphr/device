export const DEVICE_SILHOUETTE_SVG = `
<svg viewBox="0 0 240 240" xmlns="http://www.w3.org/2000/svg">
  <path d="M120 18 L206 66 V174 L120 222 L34 174 V66 Z" fill="#ffffff" />
  <circle cx="120" cy="120" r="36" fill="#ffffff" />
</svg>`;

export interface DeviceVisualParams {
  colorShift: number;
  speed: number;
  logoScale: number;
  logoOpacity: number;
}

export function deviceVisualParams(name: string): DeviceVisualParams {
  const hash = hashName(name);

  return {
    colorShift: hash % 360,
    speed: 0.35 + ((hash >>> 9) % 40) / 100,
    logoScale: 0.9 + ((hash >>> 15) % 20) / 100,
    logoOpacity: 0.9 + ((hash >>> 21) % 10) / 100,
  };
}

function hashName(name: string): number {
  let hash = 2166136261;
  for (let index = 0; index < name.length; index += 1) {
    hash ^= name.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}
