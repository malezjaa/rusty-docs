import type { BaseLayoutProps, LinkItemType } from 'fumadocs-ui/layouts/shared';
import { appName, gitConfig } from './shared';
import siteConfig from './config';

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      enabled: siteConfig.navigation.navbar,
      title: <SiteTitle />,
      transparentMode: siteConfig.navigation.navTransparent,
    },
    links: navigationLinks(),
    themeSwitch: {
      enabled: siteConfig.theme.themeSwitch,
      mode: siteConfig.theme.themeSwitchMode,
    },
    searchToggle: {
      enabled: siteConfig.navigation.search,
    },
    ...(gitConfig.githubUrl ? { githubUrl: gitConfig.githubUrl } : {}),
  };
}

function SiteTitle() {
  return (
    <span className="site-title">
      {siteConfig.logoUrl ? (
        <img
          alt=""
          aria-hidden="true"
          className={`site-logo${siteConfig.logoDarkUrl ? ' site-logo--light' : ''}`}
          src={siteConfig.logoUrl}
        />
      ) : null}
      {siteConfig.logoDarkUrl ? (
        <img
          alt=""
          aria-hidden="true"
          className="site-logo site-logo--dark"
          src={siteConfig.logoDarkUrl}
        />
      ) : null}
      <span>{appName}</span>
    </span>
  );
}

function navigationLinks(): LinkItemType[] {
  return siteConfig.navigation.links.map((link): LinkItemType => {
    const shared = {
      text: link.text,
      url: link.url,
      active: link.active,
      on: link.placement,
      external: link.external,
    } as const;

    return link.style === 'button'
      ? { ...shared, type: 'button', secondary: link.secondary }
      : { ...shared, type: 'main' };
  });
}
