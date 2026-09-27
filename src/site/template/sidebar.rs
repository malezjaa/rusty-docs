use std::{fs, path::Path};

use anyhow::{Context, Result};

pub(super) fn upgrade_api_sidebar_layout(site_dir: &Path) -> Result<()> {
    const OLD_TREE: &str = "tree={source.getPageTree()}";
    const NEW_TREE: &str = "tree={markApiItems(source.getPageTree())}";
    const HELPERS: &str = r#"

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
        ? { name: <ApiItemLabel compact kind={folderKind} name={node.name} /> }
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

function markApiItemPage(
  node: Extract<PageTreeItem, { type: 'page' }>,
): Extract<PageTreeItem, { type: 'page' }> {
  const kind = apiKindForUrl(node.url);
  return kind
    ? { ...node, name: <ApiItemLabel compact kind={kind} name={node.name} /> }
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
"#;
    const SORT_HELPERS: &str = r#"

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
"#;
    const ITEM_NODES_HELPER: &str = r#"

function markApiItemNodes(nodes: PageTreeItem[]): PageTreeItem[] {
  return sortApiItems(nodes).map((node) => markApiItemNode(node));
}
"#;
    const API_FOLDER_HELPER: &str = r#"if (node.type === 'folder') {
    const indexKind = node.index ? apiKindForUrl(node.index.url) : undefined;
    const children = markApiItemNodes(node.children);
    if (node.index && indexKind && children.length === 0) {
      return markApiItemPage({ ...node.index, type: 'page', name: node.name });
    }
    const folderKind = indexKind ?? (children.some(hasApiItem) ? 'module' : undefined);
    return {
      ...node,
      ...(folderKind
        ? { name: <ApiItemLabel compact kind={folderKind} name={node.name} /> }
        : {}),
      ...(node.index ? { index: markApiItemPage(node.index) } : {}),
      children,
    };
  }"#;

    let path = site_dir.join("app/docs/layout.tsx");
    if !path.is_file() {
        return Ok(());
    }
    let mut source = fs::read_to_string(&path)
        .with_context(|| format!("Could not read layout {}", path.display()))?;
    if source.contains("markApiItems") {
        let original = source.clone();
        const LEGACY_INDEX_MAPPING: &str = "index: markApiItemNode(node.index)";
        const CURRENT_INDEX_MAPPING: &str = "index: markApiItemPage(node.index)";
        const PAGE_HELPER: &str = r#"

function markApiItemPage(
  node: Extract<PageTreeItem, { type: 'page' }>,
): Extract<PageTreeItem, { type: 'page' }> {
  const kind = apiItemKinds.get(normalizeApiPageUrl(node.url));
  return kind
    ? { ...node, name: <ApiItemLabel compact kind={kind} name={node.name} /> }
    : node;
}
"#;
        if source.contains(LEGACY_INDEX_MAPPING) && !source.contains("function markApiItemPage(") {
            source = source.replace(LEGACY_INDEX_MAPPING, CURRENT_INDEX_MAPPING);
            source.push_str(PAGE_HELPER);
        }

        if !source.contains("const crateRoutes = new Set(") {
            const OLD_API_MAP: &str = r#"const apiItemKinds = new Map<string, string>(
  siteConfig.apiItems.map((item): [string, string] => [
    normalizeApiPageUrl(item.route),
    item.kind,
  ]),
);
"#;
            const NEW_API_MAP: &str = r#"const apiItemKinds = new Map<string, string>(
  siteConfig.apiItems.map((item): [string, string] => [
    normalizeApiPageUrl(item.route),
    item.kind,
  ]),
);
const crateRoutes = new Set(
  siteConfig.packages.map((pkg) => normalizeApiPageUrl(`/docs/api/${pkg.slug}`)),
);
"#;
            source = source.replace(OLD_API_MAP, NEW_API_MAP);
        }

        if source.contains("const crateRoutes = new Set(") {
            const OLD_FOLDER_HELPER: &str = r#"if (node.type === 'folder') {
    return {
      ...node,
      ...(node.index ? { index: markApiItemPage(node.index) } : {}),
      children: node.children.map(markApiItemNode),
    };
  }"#;
            source = source.replace(OLD_FOLDER_HELPER, API_FOLDER_HELPER);
            source = source.replace(
                "const kind = apiItemKinds.get(normalizeApiPageUrl(node.url));",
                "const kind = apiKindForUrl(node.url);",
            );
            if !source.contains("function apiKindForUrl(") {
                const API_KIND_HELPER: &str = r#"function apiKindForUrl(url: string) {
  const normalizedUrl = normalizeApiPageUrl(url);
  return crateRoutes.has(normalizedUrl)
    ? 'crate'
    : apiItemKinds.get(normalizedUrl);
}

"#;
                if let Some(insertion) = source.find("function normalizeApiPageUrl(url: string) {")
                {
                    source.insert_str(insertion, API_KIND_HELPER);
                }
            }
        }

        source = source.replace(
            "children: sortApiItems(tree.children.map(markApiItemNode))",
            "children: markApiItemNodes(tree.children)",
        );
        source = source.replace(
            "children: tree.children.map(markApiItemNode)",
            "children: markApiItemNodes(tree.children)",
        );
        source = source.replace(
            "const children = sortApiItems(node.children.map(markApiItemNode));",
            "const children = markApiItemNodes(node.children);",
        );
        source = source.replace(
            "children: sortApiItems(node.children.map(markApiItemNode)),",
            "children: markApiItemNodes(node.children),",
        );
        source = source.replace(
            "children: node.children.map(markApiItemNode),",
            "children: markApiItemNodes(node.children),",
        );
        const CURRENT_FOLDER_HELPER: &str = r#"if (node.type === 'folder') {
    const indexKind = node.index ? apiKindForUrl(node.index.url) : undefined;
    const children = markApiItemNodes(node.children);
    return {
      ...node,
      ...(indexKind
        ? { name: <ApiItemLabel compact kind={indexKind} name={node.name} /> }
        : {}),
      ...(node.index ? { index: markApiItemPage(node.index) } : {}),
      children,
    };
  }"#;
        const GROUPED_FOLDER_HELPER: &str = r#"if (node.type === 'folder') {
    const indexKind = node.index ? apiKindForUrl(node.index.url) : undefined;
    const children = markApiItemNodes(node.children);
    return {
      ...node,
      ...(indexKind
        ? {
            name: (
              <ApiItemLabel
                compact
                groupStart={groupStart}
                kind={indexKind}
                name={node.name}
              />
            ),
          }
        : {}),
      ...(node.index ? { index: markApiItemPage(node.index) } : {}),
      children,
    };
  }"#;
        source = source.replace(CURRENT_FOLDER_HELPER, API_FOLDER_HELPER);
        source = source.replace(GROUPED_FOLDER_HELPER, API_FOLDER_HELPER);
        source = source.replace(
            "function markApiItemNode(node: PageTreeItem, groupStart: boolean): PageTreeItem {",
            "function markApiItemNode(node: PageTreeItem): PageTreeItem {",
        );
        source = source.replace(
            "return markApiItemPage(node, groupStart);",
            "return markApiItemPage(node);",
        );
        source = source.replace("groupStart={groupStart} ", "");
        source = source.replace("groupStart={groupStart}\n", "");
        source = source.replace(
            "  node: Extract<PageTreeItem, { type: 'page' }>,\n  groupStart = false,\n): Extract<PageTreeItem, { type: 'page' }> {",
            "  node: Extract<PageTreeItem, { type: 'page' }>,\n): Extract<PageTreeItem, { type: 'page' }> {",
        );
        const OLD_GROUPED_ITEMS: &str = r#"function markApiItemNodes(nodes: PageTreeItem[]): PageTreeItem[] {
  const sorted = sortApiItems(nodes);
  let previousKind: string | undefined;
  return sorted.map((node) => {
    const kind = apiNodeKind(node);
    const groupStart = kind !== undefined && kind !== previousKind;
    if (kind !== undefined) previousKind = kind;
    return markApiItemNode(node, groupStart);
  });
}"#;
        source = source.replace(
            OLD_GROUPED_ITEMS,
            "function markApiItemNodes(nodes: PageTreeItem[]): PageTreeItem[] {\n  return sortApiItems(nodes).map((node) => markApiItemNode(node));\n}",
        );
        if !source.contains("function hasApiItem(") {
            source.push_str(
                r#"

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
"#,
            );
        }
        if !source.contains("function markApiItemNodes(") {
            source.push_str(ITEM_NODES_HELPER);
        }
        if !source.contains("const apiKindOrder = [") {
            source.push_str(SORT_HELPERS);
        }

        if source != original {
            fs::write(&path, source)
                .with_context(|| format!("Could not update layout {}", path.display()))?;
        }
        return Ok(());
    }
    if !source.contains(OLD_TREE) {
        return Ok(());
    }

    let imports = [
        (
            "fumadocs-core/page-tree",
            "import type * as PageTree from 'fumadocs-core/page-tree';\n",
        ),
        (
            "@/components/api-item",
            "import { ApiItemLabel } from '@/components/api-item';\n",
        ),
        (
            "@/rusty-docs-site.json",
            "import siteConfig from '@/rusty-docs-site.json';\n",
        ),
    ]
    .into_iter()
    .filter(|(marker, _)| !source.contains(marker))
    .map(|(_, import)| import)
    .collect::<Vec<_>>()
    .join("");
    let insertion = if source.starts_with("'use client';") || source.starts_with("\"use client\";")
    {
        source.find('\n').map_or(0, |index| index + 1)
    } else {
        0
    };
    source.insert_str(insertion, &imports);
    source = source.replace(OLD_TREE, NEW_TREE);
    source.push_str(HELPERS);
    source.push_str(SORT_HELPERS);
    fs::write(&path, source).with_context(|| format!("Could not update layout {}", path.display()))
}

pub(super) fn upgrade_content_sidebar_position(site_dir: &Path) -> Result<()> {
    const OLD_ROOT: &str = r#"function markApiItems(tree: PageTree.Root): PageTree.Root {
  return { ...tree, children: markApiItemNodes(tree.children) };
}"#;
    const NEW_ROOT: &str = r#"function markApiItems(tree: PageTree.Root): PageTree.Root {
  const nodes = sortApiItems(tree.children);
  const contentSection = nodes.find(isContentSection);
  const otherNodes = nodes.filter((node) => !isContentSection(node));
  const children =
    siteConfig.contentPosition === 'start'
      ? [...(contentSection ? [contentSection] : []), ...otherNodes]
      : [...otherNodes, ...(contentSection ? [contentSection] : [])];
  return { ...tree, children: children.map(markApiItemNode) };
}

function isContentSection(node: PageTreeItem): boolean {
  return (
    node.type === 'folder' &&
    ((node.index !== undefined && isContentUrl(node.index.url)) ||
      node.children.some(hasContentPage))
  );
}

function hasContentPage(node: PageTreeItem): boolean {
  if (node.type === 'page') return isContentUrl(node.url);
  if (node.type === 'folder') {
    return (
      (node.index !== undefined && isContentUrl(node.index.url)) ||
      node.children.some(hasContentPage)
    );
  }
  return false;
}

function isContentUrl(url: string): boolean {
  const path = normalizeApiPageUrl(url);
  return path === '/docs/content' || path.startsWith('/docs/content/');
}"#;

    let path = site_dir.join("app/docs/layout.tsx");
    if !path.is_file() {
        return Ok(());
    }
    let mut source = fs::read_to_string(&path)
        .with_context(|| format!("Could not read layout {}", path.display()))?;
    if source.contains("function isContentSection(") || !source.contains(OLD_ROOT) {
        return Ok(());
    }

    source = source.replace(OLD_ROOT, NEW_ROOT);
    fs::write(&path, source).with_context(|| format!("Could not update layout {}", path.display()))
}
