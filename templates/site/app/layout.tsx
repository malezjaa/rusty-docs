import { Inter } from 'next/font/google';
import { Provider } from '@/components/provider';
import { appDescription, appName, siteUrl } from '@/lib/shared';
import siteConfig from '@/lib/config';
import './global.css';

export const metadata = {
  title: appName,
  description: appDescription,
  ...(siteConfig.faviconUrl ? { icons: { icon: siteConfig.faviconUrl } } : {}),
  ...(siteUrl ? { metadataBase: new URL(siteUrl) } : {}),
};

const inter = Inter({
  subsets: ['latin'],
});

export default function Layout({ children }: LayoutProps<'/'>) {
  return (
    <html lang={siteConfig.language} className={inter.className} suppressHydrationWarning>
      <body className="flex flex-col min-h-screen" dir={siteConfig.direction}>
        <Provider>{children}</Provider>
      </body>
    </html>
  );
}
