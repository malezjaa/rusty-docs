import { source } from '@/lib/source';
import { DocsLayout } from 'fumadocs-ui/layouts/docs';
import { baseOptions } from '@/lib/layout.shared';
import type * as PageTree from 'fumadocs-core/page-tree';
import { ApiItemLabel } from '@/components/api-item';
import siteConfig from '@/rusty-docs-site.json';

type PageTreeItem = PageTree.Root['children'][number];
const apiItemKinds = new Map<string, string>(
  siteConfig.apiItems.map((item): [string, string] => [
    normalizeApiPageUrl(item.route),
    item.kind,
  ]),
);
const crateRoutes = new Set(
  siteConfig.packages.map((pkg) => normalizeApiPageUrl(`/docs/api/${pkg.slug}`)),
);

export default function Layout({ children }: LayoutProps<'/docs'>) {
  return (
    <DocsLayout tree={markApiItems(source.getPageTree())} {...baseOptions()}>
      {children}
    </DocsLayout>
  );
}

function markApiItems(tree: PageTree.Root): PageTree.Root {
  return { ...tree, children: markApiItemNodes(tree.children) };
}

function markApiItemNodes(nodes: PageTreeItem[]): PageTreeItem[] {
  const sorted = sortApiItems(nodes);
  return sorted.map((node) => markApiItemNode(node));
}

function markApiItemNode(node: PageTreeItem): PageTreeItem {
  if (node.type === 'page') {
    return markApiItemPage(node);
  }
  if (node.type === 'folder') {
    const indexKind = node.index ? apiKindForUrl(node.index.url) : undefined;
    const children = markApiItemNodes(node.children);
    if (node.index && indexKind && children.length === 0) {
      return markApiItemPage({ ...node.index, type: 'page', name: node.name });
    }
    const folderKind = indexKind ?? (children.some(hasApiItem) ? 'module' : undefined);
    return {
      ...node,
      ...(folderKind
        ? {
            name: (
              <ApiItemLabel
                compact
                kind={folderKind}
                name={node.name}
              />
            ),
          }
        : {}),
      ...(node.index ? { index: markApiItemPage(node.index) } : {}),
      children,
    };
  }
  return node;
}

function hasApiItem(node: PageTreeItem): boolean {
  if (node.type === 'page') return apiKindForUrl(node.url) !== undefined;
  if (node.type === 'folder') {
    return (
      (node.index !== undefined && apiKindForUrl(node.index.url) !== undefined) ||
      node.children.some(hasApiItem)
    );
  }
  return false;
}

const apiKindOrder = [
  'crate',
  'module',
  'struct',
  'enum',
  'union',
  'constant',
  'static',
  'function',
  'type_alias',
  'trait',
  'trait_alias',
  'macro',
  'proc_macro',
  'proc_attribute',
  'proc_derive',
  'reexport',
];

function sortApiItems(nodes: PageTreeItem[]): PageTreeItem[] {
  return [...nodes].sort((left, right) => {
    const leftRank = apiKindOrder.indexOf(apiNodeKind(left) ?? '');
    const rightRank = apiKindOrder.indexOf(apiNodeKind(right) ?? '');
    if (leftRank < 0) return rightRank < 0 ? 0 : 1;
    if (rightRank < 0) return -1;
    return leftRank - rightRank;
  });
}

function apiNodeKind(node: PageTreeItem) {
  if (node.type === 'page') return apiKindForUrl(node.url);
  if (node.type === 'folder' && node.index) return apiKindForUrl(node.index.url);
  return undefined;
}

function markApiItemPage(
  node: Extract<PageTreeItem, { type: 'page' }>,
): Extract<PageTreeItem, { type: 'page' }> {
  const kind = apiKindForUrl(node.url);
  return kind
    ? {
        ...node,
        name: (
          <ApiItemLabel
            compact
            kind={kind}
            name={node.name}
          />
        ),
      }
    : node;
}

function apiKindForUrl(url: string) {
  const normalizedUrl = normalizeApiPageUrl(url);
  return crateRoutes.has(normalizedUrl)
    ? 'crate'
    : apiItemKinds.get(normalizedUrl);
}

function normalizeApiPageUrl(url: string) {
  const path = url.split(/[?#]/, 1)[0];
  const basePath = siteConfig.basePath === '/' ? '' : siteConfig.basePath;
  const pagePath =
    basePath && path.startsWith(`${basePath}/docs/`) ? path.slice(basePath.length) : path;
  return pagePath.replace(/\/+$/, '') || '/';
}
