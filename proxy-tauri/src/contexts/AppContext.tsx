import { createContext, useContext, useState, useEffect, type ReactNode } from 'react';
import type { Lang } from '@/lib/i18n';

interface AppContextType {
  theme: 'dark' | 'light';
  setTheme: (t: 'dark' | 'light') => void;
  lang: Lang;
  setLang: (l: Lang) => void;
  proxyRunning: boolean;
  setProxyRunning: (v: boolean) => void;
}

const AppContext = createContext<AppContextType>(null!);

export function AppProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<'dark' | 'light'>(
    () => (localStorage.getItem('proxy-theme') as 'dark' | 'light') ?? 'dark'
  );
  const [lang, setLangState] = useState<Lang>(
    () => (localStorage.getItem('proxy-lang') as Lang) ?? 'zh'
  );
  const [proxyRunning, setProxyRunning] = useState(false);

  const setTheme = (t: 'dark' | 'light') => { setThemeState(t); localStorage.setItem('proxy-theme', t); };
  const setLang = (l: Lang) => { setLangState(l); localStorage.setItem('proxy-lang', l); };

  useEffect(() => {
    document.documentElement.className = theme;
  }, [theme]);

  return (
    <AppContext.Provider value={{ theme, setTheme, lang, setLang, proxyRunning, setProxyRunning }}>
      {children}
    </AppContext.Provider>
  );
}

export function useApp() { return useContext(AppContext); }
