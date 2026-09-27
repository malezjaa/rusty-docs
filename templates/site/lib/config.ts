import rawSiteConfig from '../rusty-docs-site.json';

export type SiteConfig = {
  title: string;
  description: string;
  basePath: string;
  contentPosition: 'start' | 'end';
  siteUrl: string | null;
  repositoryUrl: string | null;
  logoUrl: string | null;
  logoDarkUrl: string | null;
  faviconUrl: string | null;
  language: string;
  direction: 'ltr' | 'rtl';
  theme: {
    preset: string;
    defaultMode: 'light' | 'dark' | 'system';
    themeSwitch: boolean;
    themeSwitchMode: 'light-dark' | 'light-dark-system';
    layoutWidth: string | null;
  };
  navigation: {
    navbar: boolean;
    navTransparent: 'always' | 'top' | 'none';
    search: boolean;
    sidebar: boolean;
    sidebarCollapsible: boolean;
    sidebarDefaultOpenLevel: number;
    prefetch: boolean;
    tabs: boolean;
    tabMode: 'top' | 'auto';
    links: Array<{
      text: string;
      url: string;
      style: 'link' | 'button';
      active: 'url' | 'nested-url' | 'none';
      placement: 'nav' | 'menu' | 'all';
      external: boolean;
      secondary: boolean;
    }>;
  };
  page: {
    toc: boolean;
    tocStyle: 'normal' | 'clerk' | 'block';
    breadcrumbs: boolean;
    footer: boolean;
    copyMarkdown: boolean;
    viewMarkdown: boolean;
    editOnGithub: boolean;
  };
  home: {
    mode: 'landing' | 'redirect';
    headline: string | null;
    links: Array<{ text: string; url: string; primary: boolean }>;
  };
  api: {
    defaultImplsOpen: boolean;
    itemOrder: string[];
    sourceLinks: boolean;
  };
  packages: Array<{
    name: string;
    crateName: string;
    version: string;
    description: string;
    apiItems: number;
    slug: string;
  }>;
  apiItems: Array<{ kind: string; route: string }>;
};

const siteConfig = rawSiteConfig as SiteConfig;

export default siteConfig;
