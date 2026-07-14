import { createContext, useContext, useState, useEffect, useRef, type ReactNode } from 'react';
import { useApp } from './AppContext';
import type { Snapshot } from '@/lib/types';

interface MetricsContextType {
  snapshot: Snapshot | null;
}

const MetricsContext = createContext<MetricsContextType>({ snapshot: null });

// 检测是否在 Tauri 环境中运行
function isTauri(): boolean {
  return !!(window as any).__TAURI_INTERNALS__;
}

export function MetricsProvider({ children }: { children: ReactNode }) {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const { setProxyRunning } = useApp();
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    if (isTauri()) {
      // Tauri 环境：使用事件推送
      import('@tauri-apps/api/event').then(({ listen }) => {
        listen<Snapshot>('metrics', (event) => {
          setSnapshot(event.payload);
        }).then(fn => { unlisten = fn; });
      });

      // 初始检查
      import('@tauri-apps/api/core').then(({ invoke }) => {
        invoke<boolean>('get_proxy_status')
          .then(status => setProxyRunning(status))
          .catch(() => setProxyRunning(false));
      });
    } else {
      // 非 Tauri 环境（浏览器）：通过代理 HTTP 端点轮询
      let retries = 0;
      const MAX_RETRIES = 30; // 30 * 2s = 60s total retry window
      const poll = async () => {
        try {
          const healthRes = await fetch('http://127.0.0.1:8000/health', { signal: AbortSignal.timeout(3000) });
          if (healthRes.ok) {
            setProxyRunning(true);
            retries = 0; // reset on success
          } else {
            setProxyRunning(false);
          }
        } catch {
          retries++;
          if (retries <= MAX_RETRIES) {
            setProxyRunning(false);
          } else {
            // Stop polling after too many failures to avoid console spam
            if (intervalRef.current) {
              clearInterval(intervalRef.current);
              intervalRef.current = null;
            }
            setProxyRunning(false);
            return;
          }
        }
      };

      poll();
      intervalRef.current = setInterval(poll, 2000);
    }

    return () => {
      if (unlisten) unlisten();
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [setProxyRunning]);

  return (
    <MetricsContext.Provider value={{ snapshot }}>
      {children}
    </MetricsContext.Provider>
  );
}

export function useMetrics() { return useContext(MetricsContext); }
