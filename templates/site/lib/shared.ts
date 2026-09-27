import { createGetUrl } from 'fumadocs-core/source';
import siteConfig from '../rusty-docs-site.json';

export const appName = siteConfig.title;
export const appDescription = siteConfig.description;
export const siteUrl = siteConfig.siteUrl;
export const docsRoute = '/docs';
export const docsImageRoute = '/og/docs';
export const docsContentRoute = '/llms.mdx/docs';

export const gitConfig = {
  githubUrl: siteConfig.repositoryUrl,
};

const apiItemKinds = new Map<string, string>(
  siteConfig.apiItems.map((item): [string, string] => [
    normalizePageUrl(item.route),
    item.kind,
  ]),
);
const apiCrateRoutes = new Set(
  siteConfig.packages.map((pkg) => normalizePageUrl(`${docsRoute}/api/${pkg.slug}`)),
);

export function getApiItemKindForUrl(url: string) {
  const normalizedUrl = normalizePageUrl(url);
  return apiItemKinds.get(normalizedUrl)
    ?? (apiCrateRoutes.has(normalizedUrl) ? 'crate' : undefined);
}

export function getApiItemKindForSlugs(slugs: readonly string[]) {
  return getApiItemKindForUrl(`${docsRoute}/${slugs.join('/')}`);
}

const getContentUrl = createGetUrl(docsContentRoute);

export function getPageMarkdownUrl(page: { slugs: string[]; locale?: string }) {
  const segments = [...page.slugs, 'content.md'];

  return { segments, url: withBasePath(getContentUrl(segments, page.locale)) };
}

const getImageUrl = createGetUrl(docsImageRoute);

export function getPageImageUrl(page: { slugs: string[]; locale?: string }) {
  const segments = [...page.slugs, 'image.png'];

  return { segments, url: withBasePath(getImageUrl(segments, page.locale)) };
}

function withBasePath(path: string) {
  return siteConfig.basePath === '/' ? path : `${siteConfig.basePath}${path}`;
}

function normalizePageUrl(url: string) {
  const path = url.split(/[?#]/, 1)[0];
  const basePath = siteConfig.basePath === '/' ? '' : siteConfig.basePath;
  const pagePath =
    basePath && path.startsWith(`${basePath}${docsRoute}/`)
      ? path.slice(basePath.length)
      : path;
  return pagePath.replace(/\/+$/, '') || '/';
}
