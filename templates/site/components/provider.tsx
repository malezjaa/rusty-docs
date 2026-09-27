'use client';
import SearchDialog from '@/components/search';
import { RootProvider } from 'fumadocs-ui/provider/next';
import { type ReactNode } from 'react';
import siteConfig from '@/lib/config';

export function Provider({ children }: { children: ReactNode }) {
  return (
    <RootProvider
      dir={siteConfig.direction}
      search={{ enabled: siteConfig.navigation.search, SearchDialog }}
      theme={{
        defaultTheme: siteConfig.theme.defaultMode,
        enableSystem: siteConfig.theme.defaultMode === 'system',
      }}
    >
      {children}
    </RootProvider>
  );
}
