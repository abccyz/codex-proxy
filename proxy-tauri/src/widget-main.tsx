import React from 'react';
import ReactDOM from 'react-dom/client';
import { AppProvider } from '@/contexts/AppContext';
import { MetricsProvider } from '@/contexts/MetricsContext';
import FloatingWidget from '@/components/FloatingWidget';
import './index.css';

// 禁用右键菜单
document.addEventListener('contextmenu', (e) => e.preventDefault());

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <AppProvider>
      <MetricsProvider>
        <FloatingWidget upstreamModel="" inWidgetWindow />
      </MetricsProvider>
    </AppProvider>
  </React.StrictMode>,
);
