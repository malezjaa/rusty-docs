import { Inter } from 'next/font/google';
import { Provider } from '@/components/provider';
import { appDescription, appName, siteUrl } from '@/lib/shared';
import './global.css';

export const metadata = {
  title: appName,
  description: appDescription,
  ...(siteUrl ? { metadataBase: new URL(siteUrl) } : {}),
};

const inter = Inter({
  subsets: ['latin'],
});

export default function Layout({ children }: LayoutProps<'/'>) {
  return (
    <html lang="en" className={inter.className} suppressHydrationWarning>
      <body className="flex flex-col min-h-screen">
        <Provider>{children}</Provider>
      </body>
    </html>
  );
}
