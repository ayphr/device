import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { CloudLinkResult, CloudPairingCodeResult, DeviceIdentityResult } from '../types';

export function useCloudLink(deviceId: string, transport: 'ble' | 'serial') {
  const [status, setStatus] = useState<CloudLinkResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isStarting, setIsStarting] = useState(false);
  const [isUnlinking, setIsUnlinking] = useState(false);

  const suffix = transport === 'serial' ? 'serial' : 'ble';
  const expiresAtRef = useRef<number | null>(null);

  const unlink = useCallback(async () => {
    setIsUnlinking(true);
    setError(null);
    try {
      await invoke(`unlink_cloud_${suffix}`, { deviceId });
      expiresAtRef.current = null;
      setStatus({ state: 'unlinked', code: null, expiresInSecs: 0, message: null });
    } catch (invokeError) {
      const message = typeof invokeError === 'string' ? invokeError : String(invokeError);
      setError(message);
      throw Object.assign(new Error(message), { cause: invokeError });
    } finally {
      setIsUnlinking(false);
    }
  }, [deviceId, suffix]);

  const start = useCallback(async () => {
    setIsStarting(true);
    setError(null);
    setStatus({ state: 'requesting', code: null, expiresInSecs: 0, message: null });
    try {
      const identity = await invoke<DeviceIdentityResult>(`get_device_identity_${suffix}`, { deviceId });
      const serial = identity.serial;
      const name = identity.name?.trim() || `Device ${serial}`;

      const pairing = await invoke<CloudPairingCodeResult>('request_cloud_pairing_code', {
        serial,
        name,
      });

      const expiresAtMs = pairing.expiresAt ? Date.parse(pairing.expiresAt) : NaN;
      expiresAtRef.current = Number.isFinite(expiresAtMs)
        ? expiresAtMs
        : Date.now() + 5 * 60 * 1000;

      setStatus({
        state: 'pending',
        code: pairing.code,
        expiresInSecs: Math.max(0, Math.round((expiresAtRef.current - Date.now()) / 1000)),
        message: null,
      });
    } catch (invokeError) {
      const message = typeof invokeError === 'string' ? invokeError : String(invokeError);
      setError(message);
      setStatus({ state: 'error', code: null, expiresInSecs: 0, message });
    } finally {
      setIsStarting(false);
    }
  }, [deviceId, suffix]);

  // Poll the cloud API for a token while a pairing code is pending.
  useEffect(() => {
    if (status?.state !== 'pending' || !status.code) return;

    const code = status.code;
    let cancelled = false;

    const interval = window.setInterval(async () => {
      if (cancelled) return;

      if (expiresAtRef.current && Date.now() >= expiresAtRef.current) {
        cancelled = true;
        window.clearInterval(interval);
        setStatus({ state: 'unlinked', code: null, expiresInSecs: 0, message: 'Pairing code expired' });
        return;
      }

      try {
        const token = await invoke<string | null>('poll_cloud_pairing', { code });
        if (cancelled) return;

        if (token) {
          await invoke(`store_cloud_token_${suffix}`, { deviceId, token });
          if (cancelled) return;
          setStatus({ state: 'linked', code: null, expiresInSecs: 0, message: null });
          window.clearInterval(interval);
        } else {
          setStatus((previous) =>
            previous && previous.state === 'pending'
              ? {
                  ...previous,
                  expiresInSecs: Math.max(
                    0,
                    Math.round(((expiresAtRef.current ?? Date.now()) - Date.now()) / 1000),
                  ),
                }
              : previous,
          );
        }
      } catch (invokeError) {
        if (cancelled) return;
        const message = typeof invokeError === 'string' ? invokeError : String(invokeError);
        setError(message);
      }
    }, 2000);

    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, [status?.state, status?.code, deviceId, suffix]);

  const refresh = useCallback(async () => {
    setStatus((previous) =>
      previous?.state === 'pending'
        ? {
            ...previous,
            expiresInSecs: Math.max(
              0,
              Math.round(((expiresAtRef.current ?? Date.now()) - Date.now()) / 1000),
            ),
          }
        : previous,
    );
    return null;
  }, []);

  return { status, error, isStarting, isUnlinking, start, refresh, unlink };
}