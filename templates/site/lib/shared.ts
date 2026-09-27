import { createGetUrl } from 'fumadocs-core/source';
import siteConfig from './config';

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
    ?? (apiCrateRoutes.has(normalizedUrl) ? 'crate' : undefined)
    ?? getRustdocItemKind(url);
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

function getRustdocItemKind(url: string) {
  const [withoutFragment, fragment = ''] = url.split('#', 2);
  const path = withoutFragment.split(/[?]/, 1)[0] ?? '';
  const fileName = path.split('/').pop() ?? '';
  if (fileName === 'index.html') return 'module';

  const anchorKind = fragment.split('.', 1)[0] ?? '';
  const anchorKinds: Record<string, string> = {
    method: 'function',
    ty_method: 'function',
    tymethod: 'function',
    associatedtype: 'type_alias',
    associatedconstant: 'constant',
    variant: 'variant',
    structfield: 'field',
  };
  if (anchorKinds[anchorKind]) return anchorKinds[anchorKind];

  const pageKind = fileName.split('.', 1)[0] ?? '';
  const pageKinds: Record<string, string> = {
    struct: 'struct',
    enum: 'enum',
    union: 'union',
    trait: 'trait',
    type: 'type_alias',
    fn: 'function',
    constant: 'constant',
    static: 'static',
    macro: 'macro',
    proc_macro: 'proc_macro',
    attr: 'proc_attribute',
    derive: 'proc_derive',
    primitive: 'primitive',
  };
  return pageKinds[pageKind];
}
