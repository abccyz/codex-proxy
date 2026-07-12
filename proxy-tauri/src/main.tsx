import React from 'react';
import ReactDOM from 'react-dom/client';
import { AppProvider } from '@/contexts/AppContext';
import { MetricsProvider } from '@/contexts/MetricsContext';
import App from './App';
import './index.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <AppProvider>
      <MetricsProvider>
        <App />
      </MetricsProvider>
    </AppProvider>
  </React.StrictMode>,
);
