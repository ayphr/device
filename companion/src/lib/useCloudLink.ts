import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { CloudLinkResult } from '../types';

export function useCloudLink(deviceId: string, transport: 'ble' | 'serial') {
  const [status, setStatus] = useState<CloudLinkResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isStarting, setIsStarting] = useState(false);
  const [isUnlinking, setIsUnlinking] = useState(false);
  const inFlight = useRef(false);

  const suffix = transport === 'serial' ? 'serial' : 'ble';
  const startCommand = `start_cloud_link_${suffix}`;
  const statusCommand = `get_cloud_link_status_${suffix}`;
  const unlinkCommand = `unlink_cloud_${suffix}`;

  const refresh = useCallback(async () => {
    if (inFlight.current) return null;
    inFlight.current = true;
    try {
      const result = await invoke<CloudLinkResult>(statusCommand, { deviceId });
      setStatus(result);
      setError(null);
      return result;
    } catch (invokeError) {
      setError(typeof invokeError === 'string' ? invokeError : String(invokeError));
      return null;
    } finally {
      inFlight.current = false;
    }
  }, [deviceId, statusCommand]);

  const start = useCallback(async () => {
    setIsStarting(true);
    setError(null);
    try {
      const result = await invoke<CloudLinkResult>(startCommand, { deviceId });
      setStatus(result);
      return result;
    } catch (invokeError) {
      const message = typeof invokeError === 'string' ? invokeError : String(invokeError);
      setError(message);
      return null;
    } finally {
      setIsStarting(false);
    }
  }, [deviceId, startCommand]);

  const unlink = useCallback(async () => {
    setIsUnlinking(true);
    setError(null);
    try {
      await invoke(unlinkCommand, { deviceId });
      setStatus({ state: 'unlinked', code: null, expiresInSecs: 0, message: null });
    } catch (invokeError) {
      const message = typeof invokeError === 'string' ? invokeError : String(invokeError);
      setError(message);
      throw Object.assign(new Error(message), { cause: invokeError });
    } finally {
      setIsUnlinking(false);
    }
  }, [deviceId, unlinkCommand]);

  const isActive = status?.state === 'pending' || status?.state === 'requesting';

  useEffect(() => {
    if (!isActive) return;
    const timer = window.setInterval(() => {
      void refresh();
    }, 2000);
    return () => window.clearInterval(timer);
  }, [isActive, refresh]);

  return { status, error, isStarting, isUnlinking, start, refresh, unlink };
}
