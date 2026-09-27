import Link from 'next/link';
import { appName, appDescription } from '@/lib/shared';
import { redirect } from 'next/navigation';
import siteConfig from '@/lib/config';

export default function HomePage() {
  if (siteConfig.home.mode === 'redirect') redirect('/docs');

  return (
    <div className="home-hero">
      <h1>{siteConfig.home.headline ?? appName}</h1>
      <p>{appDescription}</p>
      <div className="home-actions">
        <Link href="/docs" className="home-link home-link--primary">
          Read the docs
        </Link>
        {siteConfig.home.links.map((link) => (
          <Link
            href={link.url}
            className={`home-link${link.primary ? ' home-link--primary' : ''}`}
            key={`${link.text}-${link.url}`}
          >
            {link.text}
          </Link>
        ))}
      </div>
    </div>
  );
}
