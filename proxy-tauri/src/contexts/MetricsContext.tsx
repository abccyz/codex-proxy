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
      const poll = async () => {
        try {
          // 先检查代理是否运行
          const healthRes = await fetch('http://127.0.0.1:8000/health');
          if (healthRes.ok) {
            setProxyRunning(true);
            // 通过代理的 metrics 端点获取数据（如果有的话）
            // 目前代理没有暴露 metrics HTTP 端点，所以这里只标记运行状态
          } else {
            setProxyRunning(false);
          }
        } catch {
          setProxyRunning(false);
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
