export const MIN_SAMPLING_INTERVAL_SECS = 1;
export const MAX_SAMPLING_INTERVAL_SECS = 6 * 60 * 60;
export const DEFAULT_SAMPLING_INTERVAL_SECS = 5;

export const MIN_UPLOAD_INTERVAL_SECS = 60;
export const MAX_UPLOAD_INTERVAL_SECS = 6 * 60 * 60;
export const DEFAULT_UPLOAD_INTERVAL_SECS = 60;

export function clampSamplingInterval(seconds: number): number {
  return Math.min(MAX_SAMPLING_INTERVAL_SECS, Math.max(MIN_SAMPLING_INTERVAL_SECS, Math.round(seconds)));
}

export function clampUploadInterval(seconds: number, samplingIntervalSecs: number): number {
  return Math.min(
    MAX_UPLOAD_INTERVAL_SECS,
    Math.max(MIN_UPLOAD_INTERVAL_SECS, Math.max(clampSamplingInterval(samplingIntervalSecs), Math.round(seconds))),
  );
}

export function validateIntervals(sampling: number, upload: number): string | null {
  if (!Number.isFinite(sampling) || sampling < MIN_SAMPLING_INTERVAL_SECS || sampling > MAX_SAMPLING_INTERVAL_SECS) {
    return `Sampling interval must be between ${MIN_SAMPLING_INTERVAL_SECS} and ${MAX_SAMPLING_INTERVAL_SECS} seconds`;
  }
  if (!Number.isFinite(upload) || upload < MIN_UPLOAD_INTERVAL_SECS || upload > MAX_UPLOAD_INTERVAL_SECS) {
    return `Upload interval must be between ${MIN_UPLOAD_INTERVAL_SECS} and ${MAX_UPLOAD_INTERVAL_SECS} seconds`;
  }
  if (upload < sampling) {
    return 'Upload interval must be at least as long as the sampling interval';
  }
  return null;
}

export function formatInterval(seconds: number): string {
  if (seconds < 60) return `${seconds} seconds`;
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  if (minutes < 60) {
    return remainingSeconds > 0 ? `${minutes}m ${remainingSeconds}s` : `${minutes} minutes`;
  }
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  if (remainingMinutes > 0) return `${hours}h ${remainingMinutes}m`;
  return `${hours} hours`;
}
